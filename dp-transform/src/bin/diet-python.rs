use std::{
    env, fs, process,
};

use dp_transform::{
    side_by_side, transform_str_to_ruff_with_options, Options,
};
use ruff_source_file::LineIndex;

const USAGE: &str =
    "usage: diet-python [--timing] [--side-by-side] <python-file>";
const SIDE_BY_SIDE_WIDTH: usize = 80;

fn main() {
    let mut timing = false;
    let mut side_by_side_output = false;
    let mut path: Option<String> = None;

    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--timing" => timing = true,
            "--side-by-side" => side_by_side_output = true,
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

    let mut result =
        match transform_str_to_ruff_with_options(&source, Options::default()) {
            Ok(result) => result,
            Err(err) => {
                eprintln!("failed to parse {}: {}", path, err);
                process::exit(1);
            }
        };
    if side_by_side_output {
        print_side_by_side(&source, &mut result.module);
    } else {
        print!("{}", result.to_string());
    }
    let timings = result.timings;

    if timing {
        eprintln!(
            "{{\"parse_ns\":{},\"rewrite_ns\":{},\"total_ns\":{}}}",
            timings.parse_time.as_nanos(),
            timings.rewrite_time.as_nanos(),
            timings.total_time.as_nanos()
        );
    }
}

fn print_side_by_side(
    source: &str,
    module: &mut ruff_python_ast::ModModule,
) {
    let line_index = LineIndex::from_source_text(source);
    let left_lines = side_by_side::source_lines(source);
    let right_entries =
        side_by_side::stmt_lines_with_locations(module, &line_index, source);
    let (right_map, right_unknown, max_right_line) =
        group_right_lines(right_entries);
    let max_line = left_lines.len().max(max_right_line);
    let num_width = max_line
        .to_string()
        .len()
        .max(2);

    for line_no in 1..=max_line {
        let left_text = left_lines
            .get(line_no - 1)
            .map(String::as_str)
            .unwrap_or("");
        let right_lines = right_map.get(&line_no);
        if let Some(lines) = right_lines {
            for (index, right_text) in lines.iter().enumerate() {
                let left_text = if index == 0 { left_text } else { "" };
                let row = format_row(
                    LineLabel::Known(line_no),
                    left_text,
                    LineLabel::Known(line_no),
                    right_text,
                    num_width,
                );
                println!("{row}");
            }
        } else {
            let row = format_row(
                LineLabel::Known(line_no),
                left_text,
                LineLabel::Missing,
                "",
                num_width,
            );
            println!("{row}");
        }
    }

    for right_text in right_unknown {
        let row = format_row(
            LineLabel::Missing,
            "",
            LineLabel::Unknown,
            &right_text,
            num_width,
        );
        println!("{row}");
    }
}

fn group_right_lines(
    entries: Vec<side_by_side::LineEntry>,
) -> (
    std::collections::BTreeMap<usize, Vec<String>>,
    Vec<String>,
    usize,
) {
    let mut map = std::collections::BTreeMap::new();
    let mut unknown = Vec::new();
    let mut max_line = 0usize;
    for entry in entries {
        match entry.line {
            Some(line) => {
                map.entry(line).or_insert_with(Vec::new).push(entry.text);
                if line > max_line {
                    max_line = line;
                }
            }
            None => {
                unknown.push(entry.text);
            }
        }
    }
    (map, unknown, max_line)
}

fn format_row(
    left_line: LineLabel,
    left_text: &str,
    right_line: LineLabel,
    right_text: &str,
    num_width: usize,
) -> String {
    let left = format_column(left_line, left_text, num_width);
    let right = format_column(right_line, right_text, num_width);
    format!("{left} || {right}")
}

#[derive(Clone, Copy)]
enum LineLabel {
    Known(usize),
    Unknown,
    Missing,
}

fn format_column(line: LineLabel, text: &str, num_width: usize) -> String {
    let line_label = match line {
        LineLabel::Known(line) => format!("{line:>num_width$}"),
        LineLabel::Unknown => format!("{:>num_width$}", "??"),
        LineLabel::Missing => " ".repeat(num_width),
    };
    let text = pad_or_truncate(text, SIDE_BY_SIDE_WIDTH);
    format!("{line_label} | {text}")
}

fn pad_or_truncate(text: &str, width: usize) -> String {
    let mut chars = text.chars();
    let mut result = String::new();
    for _ in 0..width {
        match chars.next() {
            Some(ch) => result.push(ch),
            None => break,
        }
    }

    if chars.next().is_some() {
        if width >= 3 {
            let mut trimmed = String::new();
            for ch in text.chars().take(width - 3) {
                trimmed.push(ch);
            }
            trimmed.push_str("...");
            return trimmed;
        }
    }

    let current_len = result.chars().count();
    if current_len < width {
        result.push_str(&" ".repeat(width - current_len));
    }
    result
}
