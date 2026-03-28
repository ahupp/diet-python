use super::super::{simplify_stmt_ast_once_for_blockpy, BlockPyStmtFragmentBuilder};
use super::*;
use crate::passes::ast_to_ast::context::Context;

#[test]
fn stmt_augassign_simplify_ast_desugars_before_blockpy_lowering() {
    let stmt = py_stmt!("x += y");
    let Stmt::AugAssign(aug_stmt) = stmt else {
        panic!("expected augassign stmt");
    };

    let context = Context::new("");
    let simplified = simplify_stmt_ast_once_for_blockpy(&context, Stmt::AugAssign(aug_stmt));

    assert!(!matches!(simplified.as_slice(), [Stmt::AugAssign(_)]));
}

#[test]
fn stmt_augassign_to_blockpy_uses_trait_owned_simplification_path() {
    let stmt = py_stmt!("x += y");
    let Stmt::AugAssign(aug_stmt) = stmt else {
        panic!("expected augassign stmt");
    };
    let context = Context::new("");
    let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
    let mut next_label_id = 0usize;

    aug_stmt
        .to_blockpy(&context, &mut out, None, &mut next_label_id)
        .expect("augassign lowering should succeed");

    let fragment = out.finish();
    assert!(matches!(
        fragment.body.as_slice(),
        [StructuredBlockPyStmt::Assign(_)]
    ));
}
