use ruff_python_ast::{comparable::ComparableStmt, ModModule, Stmt};
use ruff_python_parser::parse_module;

use crate::{ruff_ast_to_string, transform_str_to_ruff};

pub(crate) fn assert_transform_eq(actual: &str, expected: &str) {
    let module = transform_str_to_ruff(actual).unwrap();
    let actual_str = ruff_ast_to_string(&module.body);
    let actual_stmt: Vec<_> = module.body.iter().map(ComparableStmt::from).collect();

    let expected_ast = parse_module(expected).unwrap().into_syntax().body;
    let expected_stmt: Vec<_> = expected_ast.iter().map(ComparableStmt::from).collect();

    if actual_stmt != expected_stmt {
        println!("actual:\n {}", actual_str);
        println!("expected:\n {}", expected);
        assert!(false, "actual and expected are not equal");
    }
}

pub(crate) fn assert_ast_eq(actual: &[Stmt], expected: &[Stmt]) {
    let actual_stmt: Vec<_> = actual.iter().map(ComparableStmt::from).collect();
    let expected_stmt: Vec<_> = expected.iter().map(ComparableStmt::from).collect();
    assert_eq!(actual_stmt, expected_stmt);
}
