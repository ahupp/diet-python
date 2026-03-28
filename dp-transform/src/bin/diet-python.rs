use std::{env, fs, process};

use dp_transform::{ruff_ast_to_string, transform_str_to_ruff};
use serde_json::json;

const USAGE: &str = "usage: diet-python [--timing] <python-file>";

fn main() {
    let mut timing = false;
    let mut path: Option<String> = None;

    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--timing" => timing = true,
            "--help" | "-h" => {
                eprintln!("{}", USAGE);
                return;
            }
            _ if arg.starts_with('-') => {
                eprintln!("unknown option: {}", arg);
                eprintln!("{}", USAGE);
                process::exit(1);
            }
            _ => {
                if path.is_none() {
                    path = Some(arg);
                } else {
                    eprintln!("unexpected argument: {}", arg);
                    eprintln!("{}", USAGE);
                    process::exit(1);
                }
            }
        }
    }

    let path = path.unwrap_or_else(|| {
        eprintln!("{}", USAGE);
        process::exit(1);
    });

    let source = match fs::read_to_string(&path) {
        Ok(src) => src,
        Err(err) => {
            eprintln!("failed to read {}: {}", path, err);
            process::exit(1);
        }
    };

    let result = match transform_str_to_ruff(&source) {
        Ok(result) => result,
        Err(err) => {
            eprintln!("failed to parse {}: {}", path, err);
            process::exit(1);
        }
    };
    let rendered = result
        .pass_tracker
        .pass_ast_to_ast()
        .map(|module| ruff_ast_to_string(&module.body))
        .unwrap_or_else(|| source.clone());
    print!("{rendered}");

    if timing {
        let pass_timings = result.pass_tracker.pass_timings().collect::<Vec<_>>();
        let pass_timings = pass_timings
            .into_iter()
            .map(|pass| {
                json!({
                    "name": pass.name,
                    "elapsed_ns": pass.elapsed.as_nanos(),
                })
            })
            .collect::<Vec<_>>();
        eprintln!(
            "{}",
            json!({
                "total_ns": result.total_time.as_nanos(),
                "pass_timings": pass_timings,
            })
        );
    }
}
