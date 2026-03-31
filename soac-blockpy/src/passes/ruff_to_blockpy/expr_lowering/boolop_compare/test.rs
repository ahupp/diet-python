use crate::block_py::{BlockPyStmtFragmentBuilder, StructuredBlockPyStmt};
use crate::passes::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup;
use crate::py_expr;
use ruff_python_ast::{CmpOp, Expr};

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

#[test]
fn compare_lowering_keeps_native_compare_expr() {
    let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
    let mut next_label_id = 0usize;

    let lowered = lower_expr_into_with_setup(py_expr!("a < b"), &mut out, None, &mut next_label_id)
        .expect("expr lowering should succeed");

    assert!(
        out.finish().body.is_empty(),
        "single comparison should not need setup statements"
    );
    let Expr::Compare(compare) = lowered else {
        panic!("expected native compare expr, got {lowered:?}");
    };
    assert_eq!(compare.ops.as_ref(), &[CmpOp::Lt]);
}
