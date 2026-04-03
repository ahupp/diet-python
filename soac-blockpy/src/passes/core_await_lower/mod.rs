use crate::block_py::{
    core_runtime_positional_call_expr_with_meta, BlockPyModule, BlockPyModuleMap,
    CoreBlockPyExprWithAwaitAndYield, CoreBlockPyExprWithYield, CoreBlockPyYieldFrom, HasMeta,
    InstrExprNode, MapExpr, WithMeta,
};
use crate::passes::{CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield};

struct CoreAwaitLoweringMap;

impl MapExpr<CoreBlockPyExprWithAwaitAndYield, CoreBlockPyExprWithYield> for CoreAwaitLoweringMap {
    fn map_expr(&self, expr: CoreBlockPyExprWithAwaitAndYield) -> CoreBlockPyExprWithYield {
        match expr {
            CoreBlockPyExprWithAwaitAndYield::Await(await_expr) => {
                let meta = await_expr.meta();
                CoreBlockPyExprWithYield::YieldFrom(
                    CoreBlockPyYieldFrom::new(core_runtime_positional_call_expr_with_meta(
                        "await_iter",
                        meta.node_index.clone(),
                        meta.range,
                        vec![self.map_expr(*await_expr.value)],
                    ))
                    .with_meta(meta),
                )
            }
            CoreBlockPyExprWithAwaitAndYield::Literal(node) => node.into(),
            CoreBlockPyExprWithAwaitAndYield::BinOp(node) => node
                .map_typed_children(&mut |child| self.map_expr(child))
                .into(),
            CoreBlockPyExprWithAwaitAndYield::UnaryOp(node) => node
                .map_typed_children(&mut |child| self.map_expr(child))
                .into(),
            CoreBlockPyExprWithAwaitAndYield::Call(node) => node
                .map_typed_children(&mut |child| self.map_expr(child))
                .into(),
            CoreBlockPyExprWithAwaitAndYield::GetAttr(node) => node
                .map_typed_children(&mut |child| self.map_expr(child))
                .into(),
            CoreBlockPyExprWithAwaitAndYield::SetAttr(node) => node
                .map_typed_children(&mut |child| self.map_expr(child))
                .into(),
            CoreBlockPyExprWithAwaitAndYield::GetItem(node) => node
                .map_typed_children(&mut |child| self.map_expr(child))
                .into(),
            CoreBlockPyExprWithAwaitAndYield::SetItem(node) => node
                .map_typed_children(&mut |child| self.map_expr(child))
                .into(),
            CoreBlockPyExprWithAwaitAndYield::DelItem(node) => node
                .map_typed_children(&mut |child| self.map_expr(child))
                .into(),
            CoreBlockPyExprWithAwaitAndYield::Load(node) => node
                .map_typed_children(&mut |child| self.map_expr(child))
                .into(),
            CoreBlockPyExprWithAwaitAndYield::Store(node) => node
                .map_typed_children(&mut |child| self.map_expr(child))
                .into(),
            CoreBlockPyExprWithAwaitAndYield::Del(node) => node
                .map_typed_children(&mut |child| self.map_expr(child))
                .into(),
            CoreBlockPyExprWithAwaitAndYield::MakeCell(node) => node
                .map_typed_children(&mut |child| self.map_expr(child))
                .into(),
            CoreBlockPyExprWithAwaitAndYield::CellRefForName(node) => node.into(),
            CoreBlockPyExprWithAwaitAndYield::CellRef(node) => node.into(),
            CoreBlockPyExprWithAwaitAndYield::MakeFunction(node) => node
                .map_typed_children(&mut |child| self.map_expr(child))
                .into(),
            CoreBlockPyExprWithAwaitAndYield::Yield(node) => node
                .map_typed_children(&mut |child| self.map_expr(child))
                .into(),
            CoreBlockPyExprWithAwaitAndYield::YieldFrom(node) => node
                .map_typed_children(&mut |child| self.map_expr(child))
                .into(),
        }
    }
}

impl BlockPyModuleMap<CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield>
    for CoreAwaitLoweringMap
{
}

pub(crate) fn lower_awaits_in_core_blockpy_module(
    module: BlockPyModule<CoreBlockPyPassWithAwaitAndYield>,
) -> BlockPyModule<CoreBlockPyPassWithYield> {
    module.map_module(&CoreAwaitLoweringMap)
}

#[cfg(test)]
mod test;
