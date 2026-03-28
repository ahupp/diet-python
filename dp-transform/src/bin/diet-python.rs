use std::{env, fs, process};

use dp_transform::transform_str_to_ruff;
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
    print!("{}", result.to_string());
    let timings = result.timings;

    if timing {
        let pass_timings = timings
            .pass_times
            .iter()
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
                "parse_ns": timings.parse_time.as_nanos(),
                "rewrite_ns": timings.rewrite_time.as_nanos(),
                "total_ns": timings.total_time.as_nanos(),
                "pass_timings": pass_timings,
            })
        );
    }
}
