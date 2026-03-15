use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use dp_transform::basic_block::block_py::{
    BlockPyBlock, BlockPyCfgFragment, BlockPyModule, BlockPyStmt,
};
use dp_transform::basic_block::normalize_bb_module_for_codegen;
use dp_transform::fixture::{parse_fixture, render_fixture, FixtureBlock};
use dp_transform::{init_logging, transform_str_to_ruff_with_options, Options};
use log::{log_enabled, trace, Level};

struct SnapshotSummaryRow {
    case_name: String,
    blockpy_blocks: usize,
    clif_blocks: usize,
}

fn collect_fixtures(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    if log_enabled!(Level::Trace) {
        trace!("collect_fixtures: entering {}", root.display());
    }
    let entries = fs::read_dir(root)
        .map_err(|err| format!("failed to read directory {}: {}", root.display(), err))?;
    for entry in entries {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_fixtures(&path, out)?;
        } else if path.file_name().is_some_and(|name| {
            name.to_string_lossy().starts_with("snapshot_")
                && path.extension().is_some_and(|ext| ext == "py")
        }) {
            if log_enabled!(Level::Trace) {
                trace!("collect_fixtures: found {}", path.display());
            }
            out.push(path);
        }
    }
    Ok(())
}

fn repo_root() -> Result<PathBuf, String> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "failed to find workspace root from CARGO_MANIFEST_DIR".to_string())
}

fn snapshot_dir() -> Result<PathBuf, String> {
    Ok(repo_root()?.join("snapshot"))
}

fn fixture_root() -> Result<PathBuf, String> {
    snapshot_dir()
}

fn snapshot_output_path_for_fixture(path: &Path) -> Result<PathBuf, String> {
    let file_stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| format!("invalid fixture filename: {}", path.display()))?;
    Ok(snapshot_dir()?.join(format!("{file_stem}.py")))
}

fn qualified_case_name(path: &Path, block: &FixtureBlock) -> Result<String, String> {
    let fixture_name = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| format!("invalid fixture filename: {}", path.display()))?;
    Ok(format!("{fixture_name}::{}", block.name))
}

fn render_blockpy_snapshot(result: &dp_transform::LoweringResult) -> (String, usize, usize) {
    let blockpy = result
        .blockpy_module
        .as_ref()
        .map(dp_transform::basic_block::blockpy_module_to_string)
        .unwrap_or_else(|| "; no BlockPy module emitted".to_string());
    let blockpy_blocks = result
        .blockpy_module
        .as_ref()
        .map(count_blockpy_blocks)
        .unwrap_or(0);
    let clif_blocks = result
        .bb_module
        .as_ref()
        .map(count_clif_blocks)
        .unwrap_or(0);
    (blockpy, blockpy_blocks, clif_blocks)
}

fn count_blockpy_blocks(module: &BlockPyModule) -> usize {
    module
        .callable_defs
        .iter()
        .map(|function| count_blockpy_blocks_in_list(&function.blocks))
        .sum()
}

fn count_blockpy_blocks_in_list(blocks: &[BlockPyBlock]) -> usize {
    blocks
        .iter()
        .map(|block| {
            1 + count_blockpy_blocks_in_stmts(&block.body)
                + count_blockpy_blocks_in_term(&block.term)
        })
        .sum()
}

fn count_blockpy_blocks_in_stmts(stmts: &[BlockPyStmt]) -> usize {
    stmts
        .iter()
        .map(|stmt| match stmt {
            BlockPyStmt::If(if_stmt) => {
                count_blockpy_blocks_in_stmt_fragment(&if_stmt.body)
                    + count_blockpy_blocks_in_stmt_fragment(&if_stmt.orelse)
            }
            _ => 0,
        })
        .sum()
}

fn count_blockpy_blocks_in_stmt_fragment(
    fragment: &BlockPyCfgFragment<BlockPyStmt, dp_transform::basic_block::block_py::BlockPyTerm>,
) -> usize {
    count_blockpy_blocks_in_stmts(&fragment.body)
        + fragment
            .term
            .as_ref()
            .map_or(0, count_blockpy_blocks_in_term)
}

fn count_blockpy_blocks_in_term(term: &dp_transform::basic_block::block_py::BlockPyTerm) -> usize {
    match term {
        dp_transform::basic_block::block_py::BlockPyTerm::IfTerm(_) => 0,
        _ => 0,
    }
}

fn count_clif_blocks(module: &dp_transform::basic_block::bb_ir::BbModule) -> usize {
    let normalized = normalize_bb_module_for_codegen(module);
    normalized
        .callable_defs
        .iter()
        .map(|function| function.blocks.len())
        .sum()
}

fn write_if_changed(path: &Path, contents: &str) -> Result<(), String> {
    let existing = fs::read_to_string(path).unwrap_or_default();
    if existing != contents {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create {}: {}", parent.display(), err))?;
        }
        fs::write(path, contents).map_err(|err| format!("{}: {}", path.display(), err))?;
    }
    Ok(())
}

fn render_snapshot_python_fixture(blocks: &[FixtureBlock]) -> String {
    let mut output = String::new();
    for (index, block) in blocks.iter().enumerate() {
        if index > 0 {
            output.push('\n');
        }
        output.push_str("# ");
        output.push_str(&block.name);
        output.push_str("\n\n");
        let input = block.input.trim_matches('\n');
        if !input.is_empty() {
            output.push_str(input);
            output.push('\n');
        }
        output.push('\n');
        output.push_str("# ==\n\n");
        let output_block = block.output.trim_matches('\n');
        if !output_block.is_empty() {
            for line in output_block.lines() {
                if line.is_empty() {
                    output.push('\n');
                } else {
                    output.push_str("# ");
                    output.push_str(line);
                    output.push('\n');
                }
            }
        }
    }
    output
}

fn is_fixture_header_line(line: &str) -> bool {
    line.starts_with("# ") && line != "# ==" && line != "# -- pre-bb --" && line != "# -- bb --"
}

fn next_nonempty_line<'a>(lines: &'a [String], start: usize) -> Option<&'a str> {
    lines[start..]
        .iter()
        .map(String::as_str)
        .find(|line| !line.trim().is_empty())
}

fn is_snapshot_block_header(lines: &[String], index: usize) -> bool {
    let Some(line) = lines.get(index) else {
        return false;
    };
    if !is_fixture_header_line(line) {
        return false;
    }
    next_nonempty_line(lines, index + 1).is_some_and(|line| !line.starts_with('#'))
}

fn parse_snapshot_fixture(contents: &str) -> Result<Vec<FixtureBlock>, String> {
    let lines = contents.lines().map(str::to_string).collect::<Vec<_>>();
    let mut blocks = Vec::new();
    let mut index = 0usize;

    while index < lines.len() {
        while index < lines.len() && lines[index].trim().is_empty() {
            index += 1;
        }
        if index >= lines.len() {
            break;
        }
        if !is_snapshot_block_header(&lines, index) {
            return Err(format!(
                "unexpected content outside of snapshot fixture blocks: `{}`",
                lines[index]
            ));
        }

        let name = lines[index][2..].trim().to_string();
        index += 1;

        let mut input_lines = Vec::new();
        let mut saw_separator = false;
        while index < lines.len() {
            let line = &lines[index];
            if line.trim() == "# ==" {
                saw_separator = true;
                index += 1;
                break;
            }
            input_lines.push(line.clone());
            index += 1;
        }

        if !saw_separator {
            return Err(format!(
                "missing `# ==` separator in snapshot fixture `{name}`"
            ));
        }

        while index < lines.len() && !is_snapshot_block_header(&lines, index) {
            index += 1;
        }

        let input = if input_lines.is_empty() {
            String::new()
        } else {
            let mut text = input_lines.join("\n");
            text.push('\n');
            text
        };

        blocks.push(FixtureBlock {
            name,
            input,
            output: String::new(),
            seen_separator: true,
        });
    }

    Ok(blocks)
}

fn load_fixture_blocks(path: &Path, contents: &str) -> Result<Vec<FixtureBlock>, String> {
    if path.starts_with(snapshot_dir()?) {
        parse_snapshot_fixture(contents)
    } else {
        parse_fixture(contents)
    }
}

fn regenerate_fixture(path: &Path, summary: &mut Vec<SnapshotSummaryRow>) -> Result<(), String> {
    if log_enabled!(Level::Trace) {
        trace!("regenerate_fixture: start {}", path.display());
    }
    let contents =
        fs::read_to_string(path).map_err(|err| format!("{}: {}", path.display(), err))?;
    let mut blocks = load_fixture_blocks(path, &contents)?;
    if log_enabled!(Level::Trace) {
        trace!("regenerate_fixture: parsed {} blocks", blocks.len());
    }

    let options = Options::for_test();
    let mut snapshot_blocks = Vec::with_capacity(blocks.len());
    for block in &blocks {
        if log_enabled!(Level::Trace) {
            trace!("regenerate_fixture: transforming {}", block.name);
        }
        let transformed = transform_str_to_ruff_with_options(&block.input, options)
            .map_err(|err| format!("{}: {}", path.display(), err))?;
        let (output, blockpy_blocks, clif_blocks) = render_blockpy_snapshot(&transformed);
        summary.push(SnapshotSummaryRow {
            case_name: qualified_case_name(path, block)?,
            blockpy_blocks,
            clif_blocks,
        });
        snapshot_blocks.push(FixtureBlock {
            name: block.name.clone(),
            input: block.input.clone(),
            output: format!("{}\n", output.trim_end()),
            seen_separator: true,
        });
    }

    for block in &mut blocks {
        block.output.clear();
    }
    let rendered_fixture = render_fixture(&blocks);
    write_if_changed(path, &rendered_fixture)?;
    let snapshot_path = snapshot_output_path_for_fixture(path)?;
    let rendered_snapshot = render_snapshot_python_fixture(&snapshot_blocks);
    write_if_changed(&snapshot_path, &rendered_snapshot)?;

    Ok(())
}

fn format_python_files(paths: &[PathBuf]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }

    let mut command = Command::new("ruff");
    command.arg("format");
    for path in paths {
        command.arg(path);
    }
    let status = command
        .status()
        .map_err(|err| format!("failed to run ruff format: {}", err))?;
    if !status.success() {
        return Err(format!("ruff format failed with status {}", status));
    }

    Ok(())
}

fn write_summary(summary: &[SnapshotSummaryRow]) -> Result<(), String> {
    if summary.is_empty() {
        return Ok(());
    }

    let summary_path = snapshot_dir()?.join("snapshot_summary.txt");
    let mut total_by_name = HashMap::new();
    for row in summary {
        *total_by_name.entry(row.case_name.clone()).or_insert(0usize) += 1;
    }

    let mut seen_by_name = HashMap::new();
    let mut contents = String::new();
    for row in summary {
        let seen = seen_by_name.entry(row.case_name.clone()).or_insert(0usize);
        *seen += 1;
        let case_name = if total_by_name[&row.case_name] > 1 {
            format!("{} [{}]", row.case_name, *seen)
        } else {
            row.case_name.clone()
        };
        contents.push_str(&format!(
            "{}: blockpy={}, clif={}\n",
            case_name, row.blockpy_blocks, row.clif_blocks
        ));
    }
    write_if_changed(&summary_path, &contents)
}

fn main() -> Result<(), String> {
    init_logging();
    fs::create_dir_all(snapshot_dir()?)
        .map_err(|err| format!("failed to create snapshot dir: {}", err))?;
    let args: Vec<String> = env::args().skip(1).collect();
    let mut fixtures = Vec::new();

    if args.is_empty() {
        let root = fixture_root()?;
        collect_fixtures(&root, &mut fixtures)?;
    } else {
        for arg in args {
            fixtures.push(PathBuf::from(arg));
        }
    }

    fixtures.sort();
    let mut summary = Vec::new();
    for path in &fixtures {
        regenerate_fixture(path, &mut summary)?;
    }
    let mut python_files = fixtures.clone();
    for fixture in &fixtures {
        python_files.push(snapshot_output_path_for_fixture(fixture)?);
    }
    format_python_files(&python_files)?;
    write_summary(&summary)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{fixture_root, parse_snapshot_fixture};

    #[test]
    fn default_fixture_root_is_snapshot_dir() {
        let root = fixture_root().expect("fixture root");
        assert!(root.ends_with("snapshot"), "{root:?}");
        assert!(root.exists(), "{root:?}");
    }

    #[test]
    fn parses_rendered_snapshot_fixture_blocks() {
        let contents = r#"# sample case

x = 1

# ==

# module_init: _dp_module_init
#
# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     __dp_store_global(globals(), "x", 1)

# another case

if flag:
    y = 2

# ==

# module_init: _dp_module_init
# function _dp_module_init() [kind=function, bind=_dp_module_init, target=local, qualname=_dp_module_init]
#     if_term flag:
"#;
        let blocks = parse_snapshot_fixture(contents).expect("parse snapshot fixture");
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].name, "sample case");
        assert_eq!(blocks[0].input, "\nx = 1\n\n");
        assert_eq!(blocks[1].name, "another case");
        assert_eq!(blocks[1].input, "\nif flag:\n    y = 2\n\n");
    }
}
