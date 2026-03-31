use super::{BlockPySetupExprLowerer, RuffToBlockPyExpr};
use crate::block_py::{
    BlockPyAssign, BlockPyIf, BlockPyStmtFragmentBuilder, StructuredBlockPyStmt,
};
use crate::passes::ruff_to_blockpy::expr_lowering::fresh_setup_name;
use crate::passes::ruff_to_blockpy::LoopContext;
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr};

fn store_name(name: &str) -> ast::ExprName {
    ast::ExprName {
        id: name.into(),
        ctx: ast::ExprContext::Store,
        range: Default::default(),
        node_index: ast::AtomicNodeIndex::default(),
    }
}

fn load_name(name: &str) -> Expr {
    py_expr!("{name:id}", name = name)
}

fn assign_name<E>(target: &str, value: Expr) -> StructuredBlockPyStmt<E>
where
    E: From<Expr>,
{
    StructuredBlockPyStmt::Assign(BlockPyAssign {
        target: store_name(target),
        value: value.into(),
    })
}

pub(super) fn lower_if_expr_into<L, E>(
    lowerer: &L,
    if_expr: ast::ExprIf,
    out: &mut BlockPyStmtFragmentBuilder<E>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<Expr, String>
where
    L: BlockPySetupExprLowerer + ?Sized,
    E: RuffToBlockPyExpr,
{
    let ast::ExprIf {
        test, body, orelse, ..
    } = if_expr;
    let target = fresh_setup_name("tmp");
    let test = lowerer.lower_expr_ast_into(*test, out, loop_ctx, next_label_id)?;

    let mut body_out = BlockPyStmtFragmentBuilder::<E>::new();
    let body_value = lowerer.lower_expr_ast_into(*body, &mut body_out, loop_ctx, next_label_id)?;
    body_out.push_stmt(assign_name(&target, body_value));

    let mut orelse_out = BlockPyStmtFragmentBuilder::<E>::new();
    let orelse_value =
        lowerer.lower_expr_ast_into(*orelse, &mut orelse_out, loop_ctx, next_label_id)?;
    orelse_out.push_stmt(assign_name(&target, orelse_value));

    out.push_stmt(StructuredBlockPyStmt::If(BlockPyIf {
        test: test.into(),
        body: body_out.finish(),
        orelse: orelse_out.finish(),
    }));
    Ok(load_name(&target))
}

#[cfg(test)]
mod test;
