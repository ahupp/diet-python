use ruff_python_ast::{comparable::ComparableStmt, Stmt};
use ruff_python_parser::parse_module;

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
        println!("actual:\n {}", actual_str);
        println!("expected:\n {}", expected);
        assert!(false, "actual and expected are not equal");
    }
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

pub(crate) fn assert_ast_eq(actual: &[Stmt], expected: &[Stmt]) {
    let actual_stmt: Vec<_> = actual.iter().map(ComparableStmt::from).collect();
    let expected_stmt: Vec<_> = expected.iter().map(ComparableStmt::from).collect();
    assert_eq!(actual_stmt, expected_stmt);
}
