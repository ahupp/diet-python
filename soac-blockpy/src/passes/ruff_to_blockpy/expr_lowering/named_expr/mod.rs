use super::{BlockPySetupExprLowerer, RuffToBlockPyExpr};
use crate::block_py::{BlockPyStmtFragmentBuilder, Meta, Store, StructuredInstr, WithMeta};
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
    E: RuffToBlockPyExpr,
{
    let ast::ExprNamed { target, value, .. } = named_expr;
    let Expr::Name(target_name) = *target else {
        return Err("named expression lowering expected a name target".to_string());
    };
    let value =
        E::from_lowered_expr(lowerer.lower_expr_ast_into(*value, out, loop_ctx, next_label_id)?);
    let load_target = target_name.clone();
    let target_name = into_store_name(target_name);
    let meta = Meta::new(target_name.node_index.clone(), target_name.range);
    out.push_stmt(StructuredInstr::Expr(
        Store::new(target_name, value).with_meta(meta).into(),
    ));
    Ok(into_load_name(load_target))
}

#[cfg(test)]
mod test;
