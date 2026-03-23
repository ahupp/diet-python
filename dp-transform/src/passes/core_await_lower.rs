use crate::block_py::{
    core_positional_call_expr_with_meta, BlockPyModule, BlockPyModuleMap, CoreBlockPyAwait,
    CoreBlockPyCall, CoreBlockPyCallArg, CoreBlockPyExprWithAwaitAndYield,
    CoreBlockPyExprWithYield, CoreBlockPyKeywordArg, CoreBlockPyYield, CoreBlockPyYieldFrom,
    IntrinsicCall,
};
use crate::passes::{CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield};
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr};

#[cfg(test)]
use crate::block_py::{
    BlockPyCallableSemanticInfo, BlockPyFunction, BlockPyFunctionKind, FunctionName,
};

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
mod tests {
    use super::*;
    use crate::block_py::{
        BlockPyLabel, BlockPyStmt, BlockPyTerm, CfgBlock, CoreBlockPyExprWithAwaitAndYield,
    };
    use crate::passes::core_eval_order::make_eval_order_explicit_in_core_block;

    #[test]
    fn lowers_await_to_yield_from_await_iter() {
        let module = BlockPyModule {
            callable_defs: vec![BlockPyFunction {
                function_id: crate::block_py::FunctionId(0),
                names: FunctionName::new("f", "f", "f", "f"),
                kind: BlockPyFunctionKind::Coroutine,
                params: Default::default(),
                blocks: vec![make_eval_order_explicit_in_core_block(CfgBlock {
                    label: BlockPyLabel("start".to_string()),
                    body: Vec::new(),
                    term: BlockPyTerm::Return(CoreBlockPyExprWithAwaitAndYield::from(
                        crate::py_expr!("await foo()"),
                    )),
                    params: Vec::new(),
                    exc_edge: None,
                })],
                doc: None,
                closure_layout: None,
                facts: crate::block_py::BlockPyCallableFacts::default(),
                semantic: BlockPyCallableSemanticInfo::default(),
            }],
        };

        let lowered = lower_awaits_in_core_blockpy_module(module);
        let block = &lowered.callable_defs[0].blocks[0];
        assert_eq!(block.body.len(), 1);
        let BlockPyStmt::Assign(await_assign) = &block.body[0] else {
            panic!("expected lowered await assignment");
        };
        let BlockPyTerm::Return(CoreBlockPyExprWithYield::Name(return_name)) = &block.term else {
            panic!("expected return of lowered await temp");
        };
        assert_eq!(return_name.id, await_assign.target.id);
        let CoreBlockPyExprWithYield::YieldFrom(yield_from) = &await_assign.value else {
            panic!("expected lowered await yield from");
        };
        let CoreBlockPyExprWithYield::Call(call) = yield_from.value.as_ref() else {
            panic!("expected __dp_await_iter call");
        };
        let CoreBlockPyExprWithYield::Name(name) = call.func.as_ref() else {
            panic!("expected await helper name");
        };
        assert_eq!(name.id.as_str(), "__dp_await_iter");
    }
}
