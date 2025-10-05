use ruff_python_ast::{comparable::ComparableStmt, Stmt};
use ruff_python_parser::parse_module;
use similar::{ChangeTag, TextDiff};

use crate::transform::Options;
use crate::{ruff_ast_to_string, transform_str_to_ruff_with_options};

pub(crate) fn assert_transform_eq_ex(actual: &str, expected: &str, truthy: bool) {
    let options = Options {
        truthy,
        ..Options::for_test()
    };
    let module = transform_str_to_ruff_with_options(actual, options).unwrap();
    let actual_str = ruff_ast_to_string(&module.body);
    let actual_stmt: Vec<_> = module.body.iter().map(ComparableStmt::from).collect();

    let rerun_module = transform_str_to_ruff_with_options(&actual_str, options).unwrap();
    let rerun_stmt: Vec<_> = rerun_module.body.iter().map(ComparableStmt::from).collect();
    if actual_stmt != rerun_stmt {
        let difference = format_first_difference(&module.body, &rerun_module.body);
        panic!("transform is not idempotent: {difference}");
    }

    let expected_ast = parse_module(expected).unwrap().into_syntax().body;
    let expected_stmt: Vec<_> = expected_ast.iter().map(ComparableStmt::from).collect();

    if actual_stmt != expected_stmt {
        let diff = format_diff(expected, actual_str.as_str());
        let message = format!("expected:\n{expected}\nactual:\n{actual_str}\n\ndiff:\n{diff}");
        panic!("{message}");
    }
}

fn format_diff(expected: &str, actual: &str) -> String {
    let diff = TextDiff::from_lines(expected, actual);
    let mut formatted = String::new();

    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => '-',
            ChangeTag::Insert => '+',
            ChangeTag::Equal => ' ',
        };
        formatted.push(sign);
        formatted.push_str(change.value());
    }

    formatted
}

fn format_first_difference(actual: &[Stmt], rerun: &[Stmt]) -> String {
    let min_len = actual.len().min(rerun.len());
    for (index, (actual_stmt, rerun_stmt)) in actual.iter().zip(rerun).enumerate().take(min_len) {
        if ComparableStmt::from(actual_stmt) != ComparableStmt::from(rerun_stmt) {
            let actual_str = ruff_ast_to_string(std::slice::from_ref(actual_stmt));
            let rerun_str = ruff_ast_to_string(std::slice::from_ref(rerun_stmt));
            return format!(
                "first difference at stmt index {index}:\nactual: {actual_str}\nrerun: {rerun_str}"
            );
        }
    }

    if actual.len() != rerun.len() {
        if actual.len() > rerun.len() {
            let remainder = &actual[rerun.len()..];
            let remainder_str = ruff_ast_to_string(remainder);
            format!(
                "rerun dropped {} trailing statement(s): {remainder_str}",
                actual.len() - rerun.len()
            )
        } else {
            let remainder = &rerun[actual.len()..];
            let remainder_str = ruff_ast_to_string(remainder);
            format!(
                "rerun added {} trailing statement(s): {remainder_str}",
                rerun.len() - actual.len()
            )
        }
    } else {
        "unable to determine difference".to_string()
    }
}

pub(crate) fn assert_transform_eq(actual: &str, expected: &str) {
    assert_transform_eq_ex(actual, expected, false);
}

pub(crate) fn assert_transform_eq_truthy(actual: &str, expected: &str) {
    assert_transform_eq_ex(actual, expected, true);
}

pub(crate) fn run_transform_fixture_tests(fixture: &str) {
    use std::panic::{catch_unwind, AssertUnwindSafe};

    #[derive(Default)]
    struct Block {
        name: String,
        input: String,
        output: String,
        seen_separator: bool,
    }

    enum Section {
        Waiting,
        Block(Block),
    }

    let mut section = Section::Waiting;

    let finalize = |block: Block| {
        if !block.seen_separator {
            panic!(
                "missing `=` separator in transform fixture `{}`",
                block.name
            );
        }

        let result = catch_unwind(AssertUnwindSafe(|| {
            assert_transform_eq(block.input.as_str(), block.output.as_str());
        }));

        if let Err(err) = result {
            let message = match err.downcast::<String>() {
                Ok(msg) => Some(*msg),
                Err(err) => match err.downcast::<&'static str>() {
                    Ok(msg) => Some((*msg).to_string()),
                    Err(_) => None,
                },
            };

            if let Some(message) = message {
                panic!("transform fixture `{}` failed: {}", block.name, message);
            } else {
                panic!("transform fixture `{}` failed", block.name);
            }
        }
    };

    for raw_line in fixture.split_inclusive('\n') {
        let mut line = raw_line;
        let has_newline = line.ends_with('\n');
        if has_newline {
            line = &line[..line.len() - 1];
        }
        if line.ends_with('\r') {
            line = &line[..line.len() - 1];
        }

        let trimmed = line.trim_end();
        if trimmed.starts_with('$') && trimmed.get(..2) == Some("$ ") {
            if let Section::Block(block) = std::mem::replace(&mut section, Section::Waiting) {
                finalize(block);
            }

            let name = trimmed[2..].trim().to_string();
            section = Section::Block(Block {
                name,
                ..Block::default()
            });
            continue;
        }

        match &mut section {
            Section::Waiting => {
                if !trimmed.is_empty() {
                    panic!(
                        "unexpected content outside of transform fixtures: `{}`",
                        line
                    );
                }
            }
            Section::Block(block) => {
                if trimmed == "=" && line.trim() == "=" {
                    if block.seen_separator {
                        panic!(
                            "multiple `=` separators found in transform fixture `{}`",
                            block.name
                        );
                    }
                    block.seen_separator = true;
                } else if block.seen_separator {
                    block.output.push_str(line);
                    if has_newline {
                        block.output.push('\n');
                    }
                } else {
                    block.input.push_str(line);
                    if has_newline {
                        block.input.push('\n');
                    }
                }
            }
        }
    }

    if let Section::Block(block) = section {
        finalize(block);
    }
}

pub(crate) fn assert_ast_eq(actual: Vec<Stmt>, expected: Vec<Stmt>) {
    let actual_stmt: Vec<_> = actual.iter().map(ComparableStmt::from).collect();
    let expected_stmt: Vec<_> = expected.iter().map(ComparableStmt::from).collect();
    assert_eq!(actual_stmt, expected_stmt);
}

#[macro_export]
macro_rules! transform_fixture_test {
    ($path:literal) => {
        #[test]
        fn transform_fixture() {
            $crate::test_util::run_transform_fixture_tests(include_str!($path));
        }
    };
    ($name:ident, $path:literal) => {
        #[test]
        fn $name() {
            $crate::test_util::run_transform_fixture_tests(include_str!($path));
        }
    };
}
