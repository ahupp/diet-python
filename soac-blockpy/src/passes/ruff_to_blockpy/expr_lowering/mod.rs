use crate::block_py::BlockPyStmtFragmentBuilder;
use crate::namegen::fresh_name;
use crate::passes::ruff_to_blockpy::LoopContext;
use ruff_python_ast::Expr;

mod boolop_compare;
mod if_expr;
mod named_expr;
mod recursive;

pub(crate) trait BlockPySetupExprLowerer {
    fn lower_expr_ast_into<E>(
        &self,
        expr: Expr,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        loop_ctx: Option<&LoopContext>,
        next_label_id: &mut usize,
    ) -> Result<Expr, String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        recursive::lower_expr_ast_recursive(self, expr, out, loop_ctx, next_label_id)
    }

    fn lower_expr_into<E>(
        &self,
        expr: Expr,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        loop_ctx: Option<&LoopContext>,
        next_label_id: &mut usize,
    ) -> Result<E, String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        Ok(self
            .lower_expr_ast_into(expr, out, loop_ctx, next_label_id)?
            .into())
    }
}

pub(crate) struct AstSetupExprLowerer;

impl BlockPySetupExprLowerer for AstSetupExprLowerer {}

pub(crate) fn lower_expr_head_ast_for_blockpy(expr: Expr) -> Expr {
    expr
}

pub(crate) fn lower_expr_into_with_setup<E>(
    expr: Expr,
    out: &mut BlockPyStmtFragmentBuilder<E>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<E, String>
where
    E: From<Expr> + std::fmt::Debug,
{
    AstSetupExprLowerer.lower_expr_into(expr, out, loop_ctx, next_label_id)
}

pub(crate) fn fresh_setup_name(prefix: &str) -> String {
    fresh_name(prefix)
}
