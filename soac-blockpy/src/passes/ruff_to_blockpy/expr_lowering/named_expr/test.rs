use crate::block_py::{BlockPyStmtFragmentBuilder, StructuredBlockPyStmt};
use crate::passes::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup;
use crate::py_expr;
use ruff_python_ast::Expr;

#[test]
fn named_expr_lowering_emits_blockpy_assign_directly() {
    let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
    let mut next_label_id = 0usize;

    let lowered =
        lower_expr_into_with_setup(py_expr!("(x := y)"), &mut out, None, &mut next_label_id)
            .expect("expr lowering should succeed");

    let fragment = out.finish();
    assert!(matches!(lowered, Expr::Name(_)), "{lowered:?}");
    let [StructuredBlockPyStmt::Assign(assign)] = &fragment.body[..] else {
        panic!("expected one direct assign stmt, got {fragment:?}");
    };
    assert_eq!(assign.target.id.as_str(), "x");
}
