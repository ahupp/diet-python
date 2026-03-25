use crate::block_py::{
    core_positional_call_expr_with_meta, BlockPyModule, BlockPyModuleMap, CoreBlockPyAwait,
    CoreBlockPyCall, CoreBlockPyCallArg, CoreBlockPyExprWithAwaitAndYield,
    CoreBlockPyExprWithYield, CoreBlockPyKeywordArg, CoreBlockPyYield, CoreBlockPyYieldFrom,
    IntrinsicCall,
};
use crate::passes::{CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield};
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr};

fn expr_name(id: &str) -> ast::ExprName {
    let Expr::Name(expr) = py_expr!("{id:id}", id = id) else {
        unreachable!();
    };
    expr
}

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
                args: call
                    .args
                    .into_iter()
                    .map(|arg| match arg {
                        CoreBlockPyCallArg::Positional(value) => {
                            CoreBlockPyCallArg::Positional(lower_core_expr_awaits(value))
                        }
                        CoreBlockPyCallArg::Starred(value) => {
                            CoreBlockPyCallArg::Starred(lower_core_expr_awaits(value))
                        }
                    })
                    .collect(),
                keywords: call
                    .keywords
                    .into_iter()
                    .map(|keyword| match keyword {
                        CoreBlockPyKeywordArg::Named { arg, value } => {
                            CoreBlockPyKeywordArg::Named {
                                arg,
                                value: lower_core_expr_awaits(value),
                            }
                        }
                        CoreBlockPyKeywordArg::Starred(value) => {
                            CoreBlockPyKeywordArg::Starred(lower_core_expr_awaits(value))
                        }
                    })
                    .collect(),
            })
        }
        CoreBlockPyExprWithAwaitAndYield::Intrinsic(call) => {
            CoreBlockPyExprWithYield::Intrinsic(IntrinsicCall {
                intrinsic: call.intrinsic,
                node_index: call.node_index,
                range: call.range,
                args: call
                    .args
                    .into_iter()
                    .map(|arg| match arg {
                        CoreBlockPyCallArg::Positional(value) => {
                            CoreBlockPyCallArg::Positional(lower_core_expr_awaits(value))
                        }
                        CoreBlockPyCallArg::Starred(value) => {
                            CoreBlockPyCallArg::Starred(lower_core_expr_awaits(value))
                        }
                    })
                    .collect(),
                keywords: call
                    .keywords
                    .into_iter()
                    .map(|keyword| match keyword {
                        CoreBlockPyKeywordArg::Named { arg, value } => {
                            CoreBlockPyKeywordArg::Named {
                                arg,
                                value: lower_core_expr_awaits(value),
                            }
                        }
                        CoreBlockPyKeywordArg::Starred(value) => {
                            CoreBlockPyKeywordArg::Starred(lower_core_expr_awaits(value))
                        }
                    })
                    .collect(),
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
