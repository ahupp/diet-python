use crate::block_py::{BlockPyStmtFragmentBuilder, StructuredBlockPyStmt};
use crate::passes::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup;
use crate::py_expr;
use ruff_python_ast::Expr;

#[test]
fn if_expr_lowering_emits_blockpy_setup_directly() {
    let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
    let mut next_label_id = 0usize;

    let lowered = lower_expr_into_with_setup(
        py_expr!("a if cond else b"),
        &mut out,
        None,
        &mut next_label_id,
    )
    .expect("expr lowering should succeed");

    let fragment = out.finish();
    assert!(matches!(lowered, Expr::Name(_)), "{lowered:?}");
    let [StructuredBlockPyStmt::If(if_stmt)] = &fragment.body[..] else {
        panic!("expected one structured if stmt, got {fragment:?}");
    };
    assert!(
        if_stmt
            .body
            .body
            .iter()
            .any(|stmt| matches!(stmt, StructuredBlockPyStmt::Assign(_))),
        "{if_stmt:?}"
    );
    assert!(
        if_stmt
            .orelse
            .body
            .iter()
            .any(|stmt| matches!(stmt, StructuredBlockPyStmt::Assign(_))),
        "{if_stmt:?}"
    );
}
