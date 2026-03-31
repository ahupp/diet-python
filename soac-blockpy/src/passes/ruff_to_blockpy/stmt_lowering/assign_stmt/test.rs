use super::super::BlockPyStmtFragmentBuilder;
use super::*;
use crate::block_py::pretty::BlockPyDebugExprText;
use crate::block_py::{CoreBlockPyExprWithAwaitAndYield, StructuredBlockPyStmt};
use crate::passes::ast_to_ast::context::Context;

#[test]
fn stmt_assign_to_blockpy_emits_direct_core_setitem() {
    let stmt = py_stmt!("obj[idx] = value");
    let Stmt::Assign(assign_stmt) = stmt else {
        panic!("expected assign stmt");
    };
    let context = Context::new("");
    let mut out = BlockPyStmtFragmentBuilder::<CoreBlockPyExprWithAwaitAndYield>::new();
    let mut next_label_id = 0usize;

    assign_stmt
        .to_blockpy(&context, &mut out, None, &mut next_label_id)
        .expect("assign lowering should succeed");

    let fragment = out.finish();
    let Some(StructuredBlockPyStmt::Expr(expr)) = fragment.body.last() else {
        panic!("expected final expr stmt, got {fragment:?}");
    };
    let rendered = expr.debug_expr_text();

    assert!(rendered.contains("SetItem("), "{rendered}");
    assert!(!rendered.contains("__dp_setitem"), "{rendered}");
}
