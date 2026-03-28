use super::super::BlockPyStmtFragmentBuilder;
use super::*;
use crate::passes::ast_to_ast::{context::Context, Options};

#[test]
fn stmt_assign_to_blockpy_emits_setup_for_if_expr_rhs() {
    let stmt = py_stmt!("result = x if cond else y");
    let Stmt::Assign(assign_stmt) = stmt else {
        panic!("expected assign stmt");
    };
    let context = Context::new(Options::for_test(), "");
    let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
    let mut next_label_id = 0usize;

    assign_stmt
        .to_blockpy(&context, &mut out, None, &mut next_label_id)
        .expect("assign lowering should succeed");

    let fragment = out.finish();
    assert!(fragment.body.len() >= 2, "{fragment:?}");
    assert!(matches!(
        fragment.body.last(),
        Some(StructuredBlockPyStmt::Assign(_))
    ));
}
