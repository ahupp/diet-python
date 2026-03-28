use super::super::{simplify_stmt_ast_once_for_blockpy, BlockPyStmtFragmentBuilder};
use super::*;
use crate::passes::ast_to_ast::context::Context;

#[test]
fn stmt_with_simplify_ast_desugars_before_blockpy_lowering() {
    let stmt = py_stmt!("with cm:\n    body()");
    let Stmt::With(with_stmt) = stmt else {
        panic!("expected with stmt");
    };

    let context = Context::new("");
    let simplified = simplify_stmt_ast_once_for_blockpy(&context, Stmt::With(with_stmt));

    assert!(!matches!(simplified.as_slice(), [Stmt::With(_)]));
}

#[test]
#[should_panic(expected = "StmtTry should have already been reduced before BlockPy lowering")]
fn stmt_with_to_blockpy_simplifies_before_hitting_sequence_only_try_lowering() {
    let stmt = py_stmt!("with cm:\n    body()");
    let Stmt::With(with_stmt) = stmt else {
        panic!("expected with stmt");
    };
    let context = Context::new("");
    let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
    let mut next_label_id = 0usize;

    let _ = with_stmt.to_blockpy(&context, &mut out, None, &mut next_label_id);
}
