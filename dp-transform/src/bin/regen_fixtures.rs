use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use dp_transform::fixture::{parse_fixture, render_fixture};
use dp_transform::{ruff_ast_to_string, transform_str_to_ruff_with_options, Options};

fn collect_fixtures(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(root)
        .map_err(|err| format!("failed to read directory {}: {}", root.display(), err))?;
    for entry in entries {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_fixtures(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "txt") {
            out.push(path);
        }
    }
    Ok(())
}

fn regenerate_fixture(path: &Path) -> Result<(), String> {
    let contents =
        fs::read_to_string(path).map_err(|err| format!("{}: {}", path.display(), err))?;
    let mut blocks = parse_fixture(&contents)?;

    let options = Options::for_test();
    for block in &mut blocks {
        let module = transform_str_to_ruff_with_options(&block.input, options)
            .map_err(|err| format!("{}: {}", path.display(), err))?;
        block.output = ruff_ast_to_string(&module.body);
    }

    let rendered = render_fixture(&blocks);
    if rendered != contents {
        fs::write(path, rendered).map_err(|err| format!("{}: {}", path.display(), err))?;
    }

    Ok(())
}

fn main() -> Result<(), String> {
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
