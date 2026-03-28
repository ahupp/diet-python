use crate::block_py::{BlockPyStmtFragmentBuilder, StructuredBlockPyStmt};
use crate::passes::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup;
use crate::py_expr;
use ruff_python_ast::Expr;

#[test]
fn boolop_lowering_emits_blockpy_setup_directly() {
    let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
    let mut next_label_id = 0usize;

    let lowered =
        lower_expr_into_with_setup(py_expr!("a and b"), &mut out, None, &mut next_label_id)
            .expect("expr lowering should succeed");

    let fragment = out.finish();
    assert!(matches!(lowered, Expr::Name(_)), "{lowered:?}");
    assert!(
        fragment
            .body
            .iter()
            .any(|stmt| matches!(stmt, StructuredBlockPyStmt::Assign(_))),
        "{fragment:?}"
    );
    assert!(
        fragment
            .body
            .iter()
            .any(|stmt| matches!(stmt, StructuredBlockPyStmt::If(_))),
        "{fragment:?}"
    );
}
