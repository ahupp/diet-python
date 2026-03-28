use crate::block_py::{BlockPyModule, BlockPyModuleVisitor, BlockPyPass, PassExpr};
use crate::passes::{
    CoreBlockPyPass, CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield,
    ResolvedStorageBlockPyPass, RuffBlockPyPass,
};
use crate::transformer::Transformer;
use ruff_python_ast::{self as ast, Expr};

#[derive(Default)]
struct RuffExprShapeCollector {
    summary: crate::PassShapeSummary,
}

impl Transformer for RuffExprShapeCollector {
    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Await(_) => self.summary.contains_await = true,
            Expr::Yield(_) | Expr::YieldFrom(_) => self.summary.contains_yield = true,
            Expr::Call(call)
                if matches!(
                    call.func.as_ref(),
                    Expr::Name(ast::ExprName { id, .. }) if id.as_str() == "__dp_add"
                ) =>
            {
                self.summary.contains_dp_add = true;
            }
            _ => {}
        }
        crate::transformer::walk_expr(self, expr);
    }
}

#[derive(Default)]
struct BlockPyPassShapeCollector {
    summary: crate::PassShapeSummary,
}

impl<P> BlockPyModuleVisitor<P> for BlockPyPassShapeCollector
where
    P: BlockPyPass,
{
    fn visit_expr(&mut self, expr: &PassExpr<P>) {
        merge_pass_shape_summary(&mut self.summary, summarize_ruff_expr(&expr.clone().into()));
    }
}

fn merge_pass_shape_summary(total: &mut crate::PassShapeSummary, part: crate::PassShapeSummary) {
    total.contains_await |= part.contains_await;
    total.contains_yield |= part.contains_yield;
    total.contains_dp_add |= part.contains_dp_add;
}

fn summarize_ruff_expr(expr: &Expr) -> crate::PassShapeSummary {
    let mut expr = expr.clone();
    let mut collector = RuffExprShapeCollector::default();
    collector.visit_expr(&mut expr);
    collector.summary
}

fn summarize_blockpy_module<P>(module: &BlockPyModule<P>) -> crate::PassShapeSummary
where
    P: BlockPyPass,
{
    let mut collector = BlockPyPassShapeCollector::default();
    module.visit_module(&mut collector);
    collector.summary
}

pub(crate) fn summarize_tracked_pass_shape(
    result: &crate::LoweringResult,
    name: &str,
) -> Option<crate::PassShapeSummary> {
    if let Some(module) = result.get_pass::<BlockPyModule<RuffBlockPyPass>>(name) {
        return Some(summarize_blockpy_module(module));
    }
    if let Some(module) = result.get_pass::<BlockPyModule<CoreBlockPyPassWithAwaitAndYield>>(name) {
        return Some(summarize_blockpy_module(module));
    }
    if let Some(module) = result.get_pass::<BlockPyModule<CoreBlockPyPassWithYield>>(name) {
        return Some(summarize_blockpy_module(module));
    }
    if let Some(module) = result.get_pass::<BlockPyModule<CoreBlockPyPass>>(name) {
        return Some(summarize_blockpy_module(module));
    }
    if let Some(module) = result.get_pass::<BlockPyModule<ResolvedStorageBlockPyPass>>(name) {
        return Some(summarize_blockpy_module(module));
    }
    if let Some(module) = result.get_pass::<BlockPyModule<ResolvedStorageBlockPyPass>>(name) {
        return Some(summarize_blockpy_module(module));
    }
    None
}
