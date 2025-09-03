use std::{env, fs, process};

use ruff_python_ast::PySourceType;
use ruff_python_formatter::{format_module_ast, PyFormatOptions};
use ruff_python_parser::{parse, ParseOptions};
use ruff_python_trivia::CommentRanges;

fn main() {
    // Expect a single argument: the path to the Python source file.
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: diet-python <python-file>");
        process::exit(1);
    });

    // Read the source code from disk.
    let source = match fs::read_to_string(&path) {
        Ok(src) => src,
        Err(err) => {
            eprintln!("failed to read {}: {}", path, err);
            process::exit(1);
        }
    };

    // Parse the file using Ruff's Python parser.
    let parse_options = ParseOptions::from(PySourceType::Python);
    let parsed = match parse(&source, parse_options) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("parse error: {}", err);
            process::exit(1);
        }
    };

    // Collect comment ranges for the formatter.
    let comment_ranges = CommentRanges::from(parsed.tokens());

    // Format the parsed module and print it to stdout.
    let options = PyFormatOptions::default();
    let formatted = match format_module_ast(&parsed, &comment_ranges, &source, options) {
        Ok(formatted) => formatted,
        Err(err) => {
            eprintln!("format error: {}", err);
            process::exit(1);
        }
    };

    match formatted.print() {
        Ok(printed) => print!("{}", printed.as_code()),
        Err(err) => {
            eprintln!("print error: {}", err);
            process::exit(1);
        }
    }
}

