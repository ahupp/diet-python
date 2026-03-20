use super::block_py::{
    BlockPyModule, BlockPyModuleMap, CoreBlockPyAwait, CoreBlockPyCall, CoreBlockPyCallArg,
    CoreBlockPyExpr, CoreBlockPyExprWithoutAwait, CoreBlockPyKeywordArg, CoreBlockPyPass,
    CoreBlockPyPassWithoutAwait, CoreBlockPyYield, CoreBlockPyYieldFrom,
};
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr};

#[cfg(test)]
use super::block_py::{
    BlockPyBlock, BlockPyFunction, BlockPyFunctionKind, FunctionName, LoweredBlockPyExtra,
};

fn expr_name(id: &str) -> ast::ExprName {
    let Expr::Name(expr) = py_expr!("{id:id}", id = id) else {
        unreachable!();
    };
    expr
}

fn lower_core_expr_awaits(expr: CoreBlockPyExpr) -> CoreBlockPyExprWithoutAwait {
    match expr {
        CoreBlockPyExpr::Name(node) => CoreBlockPyExprWithoutAwait::Name(node),
        CoreBlockPyExpr::Literal(literal) => CoreBlockPyExprWithoutAwait::Literal(literal),
        CoreBlockPyExpr::Call(call) => CoreBlockPyExprWithoutAwait::Call(CoreBlockPyCall {
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
                    CoreBlockPyKeywordArg::Named { arg, value } => CoreBlockPyKeywordArg::Named {
                        arg,
                        value: lower_core_expr_awaits(value),
                    },
                    CoreBlockPyKeywordArg::Starred(value) => {
                        CoreBlockPyKeywordArg::Starred(lower_core_expr_awaits(value))
                    }
                })
                .collect(),
        }),
        CoreBlockPyExpr::Await(CoreBlockPyAwait {
            node_index,
            range,
            value,
        }) => CoreBlockPyExprWithoutAwait::YieldFrom(CoreBlockPyYieldFrom {
            node_index: node_index.clone(),
            range,
            value: Box::new(CoreBlockPyExprWithoutAwait::Call(CoreBlockPyCall {
                node_index,
                range,
                func: Box::new(CoreBlockPyExprWithoutAwait::Name(expr_name(
                    "__dp_await_iter",
                ))),
                args: vec![CoreBlockPyCallArg::Positional(lower_core_expr_awaits(
                    *value,
                ))],
                keywords: Vec::new(),
            })),
        }),
        CoreBlockPyExpr::Yield(CoreBlockPyYield {
            node_index,
            range,
            value,
        }) => CoreBlockPyExprWithoutAwait::Yield(CoreBlockPyYield {
            node_index,
            range,
            value: value.map(|value| Box::new(lower_core_expr_awaits(*value))),
        }),
        CoreBlockPyExpr::YieldFrom(CoreBlockPyYieldFrom {
            node_index,
            range,
            value,
        }) => CoreBlockPyExprWithoutAwait::YieldFrom(CoreBlockPyYieldFrom {
            node_index,
            range,
            value: Box::new(lower_core_expr_awaits(*value)),
        }),
    }
}

struct CoreAwaitLoweringMap;

impl BlockPyModuleMap<CoreBlockPyPass, CoreBlockPyPassWithoutAwait> for CoreAwaitLoweringMap {
    fn map_expr(&self, expr: CoreBlockPyExpr) -> CoreBlockPyExprWithoutAwait {
        lower_core_expr_awaits(expr)
    }
}

pub(crate) fn lower_awaits_in_core_blockpy_module(
    module: BlockPyModule<CoreBlockPyPass>,
) -> BlockPyModule<CoreBlockPyPassWithoutAwait> {
    module.map_module(&CoreAwaitLoweringMap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basic_block::block_py::{BlockPyLabel, BlockPyTerm, CoreBlockPyExpr};

    #[test]
    fn lowers_await_to_yield_from_await_iter() {
        let module = BlockPyModule {
            callable_defs: vec![BlockPyFunction {
                function_id: super::super::block_py::FunctionId(0),
                names: FunctionName::new("f", "f", "f", "f"),
                kind: BlockPyFunctionKind::Coroutine,
                params: Default::default(),
                param_defaults: Vec::new(),
                blocks: vec![BlockPyBlock {
                    label: BlockPyLabel("start".to_string()),
                    body: Vec::new(),
                    term: BlockPyTerm::Return(Some(CoreBlockPyExpr::from(crate::py_expr!(
                        "await foo()"
                    )))),
                    params: Vec::new(),
                    meta: Default::default(),
                }],
                doc: None,
                closure_layout: None,
                facts: super::super::block_py::BlockPyCallableFacts::default(),
                try_regions: Vec::new(),
                extra: LoweredBlockPyExtra::default(),
            }],
        };

        let lowered = lower_awaits_in_core_blockpy_module(module);
        let block = &lowered.callable_defs[0].blocks[0];
        let BlockPyTerm::Return(Some(CoreBlockPyExprWithoutAwait::YieldFrom(yield_from))) =
            &block.term
        else {
            panic!("expected yield from return");
        };
        let CoreBlockPyExprWithoutAwait::Call(call) = yield_from.value.as_ref() else {
            panic!("expected __dp_await_iter call");
        };
        let CoreBlockPyExprWithoutAwait::Name(name) = call.func.as_ref() else {
            panic!("expected await helper name");
        };
        assert_eq!(name.id.as_str(), "__dp_await_iter");
    }
}
