use ruff_python_ast::visitor::transformer::walk_body;
use ruff_python_ast::{comparable::ComparableStmt, Stmt};
use ruff_python_parser::parse_module;

use crate::transform::truthy::TruthyRewriter;
use crate::{ruff_ast_to_string, transform_str_to_ruff};

pub(crate) enum TransformPhase {
    Core,
    Full,
}

pub(crate) fn assert_transform_eq_ex(actual: &str, expected: &str, phase: TransformPhase) {
    let mut module = transform_str_to_ruff(actual).unwrap();
    if matches!(phase, TransformPhase::Full) {
        crate::template::flatten(&mut module.body);
        let truthy_transformer = TruthyRewriter::new();
        walk_body(&truthy_transformer, &mut module.body);
    }
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

pub(crate) fn assert_transform_eq(actual: &str, expected: &str) {
    assert_transform_eq_ex(actual, expected, TransformPhase::Core);
}

pub(crate) fn assert_ast_eq(actual: &[Stmt], expected: &[Stmt]) {
    let actual_stmt: Vec<_> = actual.iter().map(ComparableStmt::from).collect();
    let expected_stmt: Vec<_> = expected.iter().map(ComparableStmt::from).collect();
    assert_eq!(actual_stmt, expected_stmt);
}
