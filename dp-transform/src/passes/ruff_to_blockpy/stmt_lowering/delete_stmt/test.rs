use super::super::{
    lower_stmt_into_with_expr, simplify_stmt_ast_once_for_blockpy, BlockPyStmtFragmentBuilder,
};
use super::*;
use crate::passes::ast_to_ast::{context::Context, Options};

#[test]
fn stmt_delete_simplify_ast_desugars_attribute_delete_before_blockpy_lowering() {
    let stmt = py_stmt!("del obj.attr");
    let Stmt::Delete(delete_stmt) = stmt else {
        panic!("expected delete stmt");
    };

    let context = Context::new(Options::for_test(), "");
    let simplified = simplify_stmt_ast_once_for_blockpy(&context, Stmt::Delete(delete_stmt));

    assert!(!matches!(simplified.as_slice(), [Stmt::Delete(_)]));
}

#[test]
fn stmt_delete_lowering_uses_trait_owned_simplification_path() {
    let stmt = py_stmt!("del obj.attr");
    let Stmt::Delete(delete_stmt) = stmt else {
        panic!("expected delete stmt");
    };
    let context = Context::new(Options::for_test(), "");
    let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
    let mut next_label_id = 0usize;
    let simplified = simplify_stmt_ast_once_for_blockpy(&context, Stmt::Delete(delete_stmt));
    for stmt in simplified {
        lower_stmt_into_with_expr(&context, &stmt, &mut out, None, &mut next_label_id)
            .expect("delete lowering should succeed");
    }

    let fragment = out.finish();
    assert!(matches!(fragment.body.as_slice(), [BlockPyStmt::Expr(_)]));
}
