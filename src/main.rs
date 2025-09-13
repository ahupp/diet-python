use std::{env, fs, process};

use diet_python::{parse_transforms, transform_to_string};

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: diet-python <python-file>");
        process::exit(1);
    });

    let source = match fs::read_to_string(&path) {
        Ok(src) => src,
        Err(err) => {
            eprintln!("failed to read {}: {}", path, err);
            process::exit(1);
        }
    };

    let transforms = parse_transforms();
    let output = match transform_to_string(&source, transforms.as_ref(), true) {
        Ok(output) => output,
        Err(err) => {
            eprintln!("failed to parse {}: {}", path, err);
            process::exit(1);
        }
    };
    print!("{}", output);
}
