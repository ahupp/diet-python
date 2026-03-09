use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

    let options = Options::for_test();
    let mut outputs = Vec::with_capacity(blocks.len());
    for block in &blocks {
        if log_enabled!(Level::Trace) {
            trace!("regenerate_fixture: transforming {}", block.name);
        }
        let transformed = transform_str_to_ruff_with_options(&block.input, options)
            .map_err(|err| format!("{}: {}", path.display(), err))?;
        outputs.push(transformed.to_string());
    }

    for (index, block) in blocks.iter_mut().enumerate() {
        block.output = outputs[index].trim_end().to_string();
        block.output.push('\n');
    }

    let rendered = render_fixture(&blocks);
    if rendered != contents {
        fs::write(path, rendered).map_err(|err| format!("{}: {}", path.display(), err))?;
    }

    Ok(())
}

fn format_fixtures(paths: &[PathBuf]) -> Result<(), String> {
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
    for path in &fixtures {
        regenerate_fixture(&path)?;
    }
    format_fixtures(&fixtures)?;

    Ok(())
}
