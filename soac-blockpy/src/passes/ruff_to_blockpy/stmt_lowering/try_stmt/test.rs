use super::super::simplify_stmt_ast_once_for_blockpy;
use super::*;
use crate::passes::ast_to_ast::context::Context;

#[test]
fn stmt_try_simplify_ast_rewrites_typed_except_before_blockpy_lowering() {
    let stmt = py_stmt!(
        r#"
try:
    work()
except ValueError as exc:
    handle(exc)
"#
    );
    let Stmt::Try(try_stmt) = stmt else {
        panic!("expected try stmt");
    };

    let context = Context::new("");
    let simplified = simplify_stmt_ast_once_for_blockpy(&context, Stmt::Try(try_stmt));
    let rendered = crate::ruff_ast_to_string(simplified.as_slice());

    assert!(
        rendered.contains("__soac__.exception_matches"),
        "{rendered}"
    );
    assert!(
        rendered.contains("__soac__.current_exception()"),
        "{rendered}"
    );
    assert!(rendered.contains("del_quietly(exc)"), "{rendered}");
}

#[test]
fn stmt_try_simplify_ast_rewrites_except_star_before_blockpy_lowering() {
    let stmt = py_stmt!(
        r#"
try:
    work()
except* ValueError as exc:
    handle(exc)
"#
    );
    let Stmt::Try(try_stmt) = stmt else {
        panic!("expected try stmt");
    };

    let context = Context::new("");
    let simplified = simplify_stmt_ast_once_for_blockpy(&context, Stmt::Try(try_stmt));
    let rendered = crate::ruff_ast_to_string(simplified.as_slice());

    assert!(
        rendered.contains("__soac__.exceptiongroup_split"),
        "{rendered}"
    );
    assert!(rendered.contains("del_quietly(exc)"), "{rendered}");
}
