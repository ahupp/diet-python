use std::{env, fs, process};

use dp_transform::transform_to_string_without_attribute_lowering_with_timing;

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

    let (output, timings) =
        match transform_to_string_without_attribute_lowering_with_timing(&source, true) {
            Ok(result) => result,
            Err(err) => {
                eprintln!("failed to parse {}: {}", path, err);
                process::exit(1);
            }
        };
    print!("{}", output);

    if timing {
        eprintln!(
            "{{\"parse_ns\":{},\"rewrite_ns\":{},\"ensure_import_ns\":{},\"emit_ns\":{},\"total_ns\":{}}}",
            timings.parse.as_nanos(),
            timings.rewrite.as_nanos(),
            timings.ensure_import.as_nanos(),
            timings.emit.as_nanos(),
            timings.total.as_nanos()
        );
    }
}
