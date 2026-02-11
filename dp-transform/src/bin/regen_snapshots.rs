use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use dp_transform::fixture::{parse_fixture, render_fixture};
use dp_transform::{init_logging, transform_str_to_ruff_with_options, Options};
use log::{log_enabled, trace, Level};

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

fn regenerate_fixture(path: &Path) -> Result<(), String> {
    if log_enabled!(Level::Trace) {
        trace!("regenerate_fixture: start {}", path.display());
    }
    let contents =
        fs::read_to_string(path).map_err(|err| format!("{}: {}", path.display(), err))?;
    let mut blocks = parse_fixture(&contents)?;
    if log_enabled!(Level::Trace) {
        trace!("regenerate_fixture: parsed {} blocks", blocks.len());
    }

    let mut pre_bb_options = Options::for_test();
    pre_bb_options.lower_basic_blocks = true;
    pre_bb_options.emit_basic_blocks = false;
    let mut bb_options = Options::for_test();
    bb_options.lower_basic_blocks = true;
    let mut bb_outputs = Vec::with_capacity(blocks.len());
    for block in &blocks {
        if log_enabled!(Level::Trace) {
            trace!("regenerate_fixture: transforming bb {}", block.name);
        }
        let bb = transform_str_to_ruff_with_options(&block.input, bb_options)
            .map_err(|err| format!("{}: {}", path.display(), err))?;
        bb_outputs.push(bb.to_string());
    }

    let mut pre_bb_outputs = Vec::with_capacity(blocks.len());
    for block in &blocks {
        if log_enabled!(Level::Trace) {
            trace!("regenerate_fixture: transforming pre-bb {}", block.name);
        }
        let pre_bb = transform_str_to_ruff_with_options(&block.input, pre_bb_options)
            .map_err(|err| format!("{}: {}", path.display(), err))?;
        pre_bb_outputs.push(pre_bb.to_string());
    }

    for (index, block) in blocks.iter_mut().enumerate() {
        block.output = format!(
            "# -- pre-bb --\n{pre}\n\n# -- bb --\n{bb}\n",
            pre = pre_bb_outputs[index].trim_end(),
            bb = bb_outputs[index].trim_end()
        );
    }

    let rendered = render_fixture(&blocks);
    if rendered != contents {
        fs::write(path, rendered).map_err(|err| format!("{}: {}", path.display(), err))?;
    }

    Ok(())
}

fn main() -> Result<(), String> {
    init_logging();
    let args: Vec<String> = env::args().skip(1).collect();
    let mut fixtures = Vec::new();

    if args.is_empty() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/transform");
        collect_fixtures(&root, &mut fixtures)?;
    } else {
        for arg in args {
            fixtures.push(PathBuf::from(arg));
        }
    }

    fixtures.sort();
    for path in fixtures {
        regenerate_fixture(&path)?;
    }

    Ok(())
}
