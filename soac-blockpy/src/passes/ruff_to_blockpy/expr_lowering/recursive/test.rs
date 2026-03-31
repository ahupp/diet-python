use crate::block_py::{
    pretty::BlockPyDebugExprText, BlockPyStmtFragmentBuilder, CoreBlockPyExprWithAwaitAndYield,
    StructuredBlockPyStmt,
};
use crate::passes::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup;
use crate::py_expr;
use ruff_python_ast::Expr;

#[test]
fn nested_boolop_in_call_argument_emits_setup_via_expr_lowering() {
    let mut out = BlockPyStmtFragmentBuilder::<Expr>::new();
    let mut next_label_id = 0usize;

    let lowered: Expr =
        lower_expr_into_with_setup(py_expr!("f(a and b)"), &mut out, None, &mut next_label_id)
            .expect("expr lowering should succeed");

    let fragment = out.finish();
    assert!(
        fragment
            .body
            .iter()
            .any(|stmt| matches!(stmt, StructuredBlockPyStmt::If(_))),
        "{fragment:?}"
    );
    let rendered = crate::ruff_ast_to_string(&lowered);
    assert!(rendered.starts_with("f(_dp_target_"), "{rendered}");
}

#[test]
fn direct_core_expr_lowering_materializes_make_function_operation() {
    let mut out = BlockPyStmtFragmentBuilder::<CoreBlockPyExprWithAwaitAndYield>::new();
    let mut next_label_id = 0usize;

    let lowered = lower_expr_into_with_setup(
        py_expr!("__dp_make_function(7, \"function\", __dp_tuple(), __dp_tuple(), None)"),
        &mut out,
        None,
        &mut next_label_id,
    )
    .expect("expr lowering should succeed");

    assert!(
        out.finish().body.is_empty(),
        "make_function should not need setup"
    );
    let rendered = lowered.debug_expr_text();
    assert!(rendered.contains("MakeFunction("), "{rendered}");
    assert!(!rendered.contains("__dp_make_function("), "{rendered}");
}
