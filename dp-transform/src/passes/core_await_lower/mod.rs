use crate::block_py::{
    core_positional_call_expr_with_meta, map_call_args_with, map_intrinsic_args_with,
    map_keyword_args_with, BlockPyModule, BlockPyModuleMap, CoreBlockPyAwait, CoreBlockPyCall,
    CoreBlockPyExprWithAwaitAndYield, CoreBlockPyExprWithYield, CoreBlockPyYield,
    CoreBlockPyYieldFrom, IntrinsicCall,
};
use crate::passes::{CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield};

fn lower_core_expr_awaits(expr: CoreBlockPyExprWithAwaitAndYield) -> CoreBlockPyExprWithYield {
    match expr {
        CoreBlockPyExprWithAwaitAndYield::Name(node) => CoreBlockPyExprWithYield::Name(node),
        CoreBlockPyExprWithAwaitAndYield::Literal(literal) => {
            CoreBlockPyExprWithYield::Literal(literal)
        }
        CoreBlockPyExprWithAwaitAndYield::Call(call) => {
            CoreBlockPyExprWithYield::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(lower_core_expr_awaits(*call.func)),
                args: map_call_args_with(call.args, lower_core_expr_awaits),
                keywords: map_keyword_args_with(call.keywords, lower_core_expr_awaits),
            })
        }
        CoreBlockPyExprWithAwaitAndYield::Intrinsic(call) => {
            CoreBlockPyExprWithYield::Intrinsic(IntrinsicCall {
                intrinsic: call.intrinsic,
                node_index: call.node_index,
                range: call.range,
                args: map_intrinsic_args_with(call.args, lower_core_expr_awaits),
            })
        }
        CoreBlockPyExprWithAwaitAndYield::Await(CoreBlockPyAwait {
            node_index,
            range,
            value,
        }) => CoreBlockPyExprWithYield::YieldFrom(CoreBlockPyYieldFrom {
            node_index: node_index.clone(),
            range,
            value: Box::new(core_positional_call_expr_with_meta(
                "__dp_await_iter",
                node_index,
                range,
                vec![lower_core_expr_awaits(*value)],
            )),
        }),
        CoreBlockPyExprWithAwaitAndYield::Yield(CoreBlockPyYield {
            node_index,
            range,
            value,
        }) => CoreBlockPyExprWithYield::Yield(CoreBlockPyYield {
            node_index,
            range,
            value: value.map(|value| Box::new(lower_core_expr_awaits(*value))),
        }),
        CoreBlockPyExprWithAwaitAndYield::YieldFrom(CoreBlockPyYieldFrom {
            node_index,
            range,
            value,
        }) => CoreBlockPyExprWithYield::YieldFrom(CoreBlockPyYieldFrom {
            node_index,
            range,
            value: Box::new(lower_core_expr_awaits(*value)),
        }),
    }
}

struct CoreAwaitLoweringMap;

impl BlockPyModuleMap<CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield>
    for CoreAwaitLoweringMap
{
    fn map_expr(&self, expr: CoreBlockPyExprWithAwaitAndYield) -> CoreBlockPyExprWithYield {
        lower_core_expr_awaits(expr)
    }
}

pub(crate) fn lower_awaits_in_core_blockpy_module(
    module: BlockPyModule<CoreBlockPyPassWithAwaitAndYield>,
) -> BlockPyModule<CoreBlockPyPassWithYield> {
    module.map_module(&CoreAwaitLoweringMap)
}

#[cfg(test)]
mod test;
