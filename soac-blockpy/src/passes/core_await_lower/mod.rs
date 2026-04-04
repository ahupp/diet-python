use crate::block_py::{
    core_runtime_positional_call_expr_with_meta, map_module, BlockPyModule,
    try_lower_core_expr_without_await_with_mapper, CoreBlockPyExprWithAwaitAndYield,
    CoreBlockPyExprWithYield, HasMeta, InstrExprNode, MapExpr, TryMapExpr, UnresolvedName,
    WithMeta, YieldFrom,
};
use crate::block_py::__soac_match_default_CoreBlockPyExprWithAwaitAndYield;
use crate::passes::{CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield};

pub(crate) struct ErrOnAwait;

impl
    TryMapExpr<
        CoreBlockPyExprWithAwaitAndYield,
        CoreBlockPyExprWithYield,
        CoreBlockPyExprWithAwaitAndYield,
    > for ErrOnAwait
{
    fn try_map_expr(
        &mut self,
        expr: CoreBlockPyExprWithAwaitAndYield,
    ) -> Result<CoreBlockPyExprWithYield, CoreBlockPyExprWithAwaitAndYield> {
        try_lower_core_expr_without_await_with_mapper(expr, self)
    }

    fn try_map_name(
        &mut self,
        name: UnresolvedName,
    ) -> Result<UnresolvedName, CoreBlockPyExprWithAwaitAndYield> {
        Ok(name)
    }
}

struct CoreAwaitLoweringMap;

impl MapExpr<CoreBlockPyExprWithAwaitAndYield, CoreBlockPyExprWithYield> for CoreAwaitLoweringMap {
    fn map_expr(&mut self, expr: CoreBlockPyExprWithAwaitAndYield) -> CoreBlockPyExprWithYield {
        __soac_match_default_CoreBlockPyExprWithAwaitAndYield!(
            expr,
            [Await],
            {
                CoreBlockPyExprWithAwaitAndYield::Await(node) => {
                    let meta = node.meta();
                    CoreBlockPyExprWithYield::YieldFrom(
                        YieldFrom::new(core_runtime_positional_call_expr_with_meta(
                            "await_iter",
                            meta.node_index.clone(),
                            meta.range,
                            vec![self.map_expr(*node.value)],
                        ))
                        .with_meta(meta),
                    )
                },
                match_rest(node) => node.map_typed_children(self).into(),
            }
        )
    }

    fn map_name(&mut self, name: UnresolvedName) -> UnresolvedName {
        name
    }
}

pub(crate) fn lower_awaits_in_core_blockpy_module(
    module: BlockPyModule<CoreBlockPyPassWithAwaitAndYield>,
) -> BlockPyModule<CoreBlockPyPassWithYield> {
    let mut mapper = CoreAwaitLoweringMap;
    map_module(&mut mapper, module)
}

#[cfg(test)]
mod test;
