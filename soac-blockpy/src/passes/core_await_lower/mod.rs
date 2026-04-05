use crate::block_py::{
    core_runtime_positional_call_expr_with_meta, map_module, BlockPyModule,
    CoreBlockPyExprWithAwaitAndYield, CoreBlockPyExprWithYield, HasMeta, MapInstr, Mappable,
    UnresolvedName, WithMeta, YieldFrom,
};
use crate::passes::{CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield};
use soac_macros::match_default;

struct CoreAwaitLoweringMap;

impl MapInstr<CoreBlockPyExprWithAwaitAndYield, CoreBlockPyExprWithYield> for CoreAwaitLoweringMap {
    fn map_instr(&mut self, expr: CoreBlockPyExprWithAwaitAndYield) -> CoreBlockPyExprWithYield {
        match_default!(expr: crate::passes::CoreBlockPyExprWithAwaitAndYield {
            CoreBlockPyExprWithAwaitAndYield::Await(node) => {
                let meta = node.meta();
                CoreBlockPyExprWithYield::YieldFrom(
                    YieldFrom::new(core_runtime_positional_call_expr_with_meta(
                        "await_iter",
                        meta.node_index.clone(),
                        meta.range,
                        vec![self.map_instr(*node.value)],
                    ))
                    .with_meta(meta),
                )
            },
            rest => rest.map_typed_children(self).into(),
        })
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
