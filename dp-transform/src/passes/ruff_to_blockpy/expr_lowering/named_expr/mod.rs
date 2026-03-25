use super::BlockPySetupExprLowerer;
use crate::block_py::{BlockPyAssign, BlockPyStmt, BlockPyStmtFragmentBuilder};
use crate::passes::ruff_to_blockpy::LoopContext;
use ruff_python_ast::{self as ast, Expr};

fn into_store_name(name: ast::ExprName) -> ast::ExprName {
    ast::ExprName {
        ctx: ast::ExprContext::Store,
        ..name
    }
}

fn into_load_name(name: ast::ExprName) -> Expr {
    Expr::Name(ast::ExprName {
        ctx: ast::ExprContext::Load,
        ..name
    })
}

pub(super) fn lower_named_expr_into<L, E>(
    lowerer: &L,
    named_expr: ast::ExprNamed,
    out: &mut BlockPyStmtFragmentBuilder<E>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<Expr, String>
where
    L: BlockPySetupExprLowerer + ?Sized,
    E: From<Expr> + std::fmt::Debug,
{
    let ast::ExprNamed { target, value, .. } = named_expr;
    let Expr::Name(target_name) = *target else {
        return Err("named expression lowering expected a name target".to_string());
    };
    let value = lowerer.lower_expr_ast_into(*value, out, loop_ctx, next_label_id)?;
    out.push_stmt(BlockPyStmt::Assign(BlockPyAssign {
        target: into_store_name(target_name.clone()),
        value: value.into(),
    }));
    Ok(into_load_name(target_name))
}

#[cfg(test)]
mod test;
