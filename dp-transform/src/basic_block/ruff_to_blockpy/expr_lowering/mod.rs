use crate::basic_block::ast_to_ast::ast_rewrite::LoweredExpr;
use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::ast_to_ast::rewrite_expr;
use crate::basic_block::block_py::BlockPyStmtFragmentBuilder;
use crate::basic_block::ruff_to_blockpy::LoopContext;
use crate::basic_block::stmt_utils::flatten_stmt_boxes;
use ruff_python_ast::Expr;

mod boolop_compare;
mod recursive;

pub(crate) trait BlockPySetupExprLowerer {
    fn simplify_expr_ast(&self, context: &Context, expr: Expr) -> LoweredExpr;

    fn lower_expr_ast_into<E>(
        &self,
        context: &Context,
        expr: Expr,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        loop_ctx: Option<&LoopContext>,
        next_label_id: &mut usize,
    ) -> Result<Expr, String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        recursive::lower_expr_ast_recursive(self, context, expr, out, loop_ctx, next_label_id)
    }

    fn lower_setup_expr<E>(
        &self,
        context: &Context,
        lowered: LoweredExpr,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        loop_ctx: Option<&LoopContext>,
        next_label_id: &mut usize,
    ) -> Result<Expr, String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        for stmt in flatten_stmt_boxes(&[Box::new(lowered.stmt)]) {
            crate::basic_block::ruff_to_blockpy::stmt_lowering::lower_nested_stmt_into_with_expr(
                context,
                stmt.as_ref(),
                out,
                loop_ctx,
                next_label_id,
            )?;
        }
        Ok(lowered.expr)
    }

    fn lower_expr_into<E>(
        &self,
        context: &Context,
        expr: Expr,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        loop_ctx: Option<&LoopContext>,
        next_label_id: &mut usize,
    ) -> Result<E, String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        Ok(self
            .lower_expr_ast_into(context, expr, out, loop_ctx, next_label_id)?
            .into())
    }
}

pub(crate) struct AstSetupExprLowerer;

impl BlockPySetupExprLowerer for AstSetupExprLowerer {
    fn simplify_expr_ast(&self, context: &Context, expr: Expr) -> LoweredExpr {
        match expr {
            Expr::Named(_)
            | Expr::If(_)
            | Expr::Lambda(_)
            | Expr::Generator(_)
            | Expr::ListComp(_)
            | Expr::SetComp(_)
            | Expr::DictComp(_) => rewrite_expr::lower_expr(context, expr),
            other => LoweredExpr::unmodified(other),
        }
    }
}

pub(crate) fn lower_expr_head_ast_for_blockpy(context: &Context, expr: Expr) -> LoweredExpr {
    match expr {
        Expr::BoolOp(bool_op) => boolop_compare::expr_boolop_to_stmts(context, bool_op),
        Expr::Compare(compare) => boolop_compare::expr_compare_to_stmts(context, compare),
        other => AstSetupExprLowerer.simplify_expr_ast(context, other),
    }
}

pub(crate) fn lower_expr_into_with_setup<E>(
    context: &Context,
    expr: Expr,
    out: &mut BlockPyStmtFragmentBuilder<E>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<E, String>
where
    E: From<Expr> + std::fmt::Debug,
{
    AstSetupExprLowerer.lower_expr_into(context, expr, out, loop_ctx, next_label_id)
}
