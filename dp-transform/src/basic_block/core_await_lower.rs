use super::block_py::{
    BlockPyAssign, BlockPyBlock, BlockPyBranchTable, BlockPyFunction, BlockPyIf, BlockPyIfTerm,
    BlockPyRaise, BlockPyStmt, BlockPyStmtFragment, BlockPyTerm, CoreBlockPyAwait, CoreBlockPyCall,
    CoreBlockPyCallArg, CoreBlockPyExpr, CoreBlockPyExprWithoutAwait, CoreBlockPyKeywordArg,
    CoreBlockPyPass, CoreBlockPyPassWithoutAwait, CoreBlockPyYield, CoreBlockPyYieldFrom,
};
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr};

#[cfg(test)]
use super::block_py::{BlockPyFunctionKind, BlockPyModule, FunctionName, LoweredBlockPyExtra};

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

fn lower_core_stmt_awaits(
    stmt: BlockPyStmt<CoreBlockPyExpr>,
) -> BlockPyStmt<CoreBlockPyExprWithoutAwait> {
    match stmt {
        BlockPyStmt::Assign(assign) => BlockPyStmt::Assign(BlockPyAssign {
            target: assign.target,
            value: lower_core_expr_awaits(assign.value),
        }),
        BlockPyStmt::Expr(expr) => BlockPyStmt::Expr(lower_core_expr_awaits(expr)),
        BlockPyStmt::Delete(delete) => BlockPyStmt::Delete(delete),
        BlockPyStmt::If(if_stmt) => BlockPyStmt::If(BlockPyIf {
            test: lower_core_expr_awaits(if_stmt.test),
            body: lower_core_fragment_awaits(if_stmt.body),
            orelse: lower_core_fragment_awaits(if_stmt.orelse),
        }),
    }
}

fn lower_core_term_awaits(
    term: BlockPyTerm<CoreBlockPyExpr>,
) -> BlockPyTerm<CoreBlockPyExprWithoutAwait> {
    match term {
        BlockPyTerm::Jump(jump) => BlockPyTerm::Jump(jump),
        BlockPyTerm::TryJump(jump) => BlockPyTerm::TryJump(jump),
        BlockPyTerm::IfTerm(if_term) => BlockPyTerm::IfTerm(BlockPyIfTerm {
            test: lower_core_expr_awaits(if_term.test),
            then_label: if_term.then_label,
            else_label: if_term.else_label,
        }),
        BlockPyTerm::BranchTable(branch) => BlockPyTerm::BranchTable(BlockPyBranchTable {
            index: lower_core_expr_awaits(branch.index),
            targets: branch.targets,
            default_label: branch.default_label,
        }),
        BlockPyTerm::Raise(BlockPyRaise { exc }) => BlockPyTerm::Raise(BlockPyRaise {
            exc: exc.map(lower_core_expr_awaits),
        }),
        BlockPyTerm::Return(value) => BlockPyTerm::Return(value.map(lower_core_expr_awaits)),
    }
}

fn lower_core_fragment_awaits(
    fragment: BlockPyStmtFragment<CoreBlockPyExpr>,
) -> BlockPyStmtFragment<CoreBlockPyExprWithoutAwait> {
    BlockPyStmtFragment {
        body: fragment
            .body
            .into_iter()
            .map(lower_core_stmt_awaits)
            .collect(),
        term: fragment.term.map(lower_core_term_awaits),
    }
}

fn lower_core_block_awaits(
    block: BlockPyBlock<CoreBlockPyExpr>,
) -> BlockPyBlock<CoreBlockPyExprWithoutAwait> {
    BlockPyBlock {
        label: block.label,
        body: block.body.into_iter().map(lower_core_stmt_awaits).collect(),
        term: lower_core_term_awaits(block.term),
        meta: block.meta,
    }
}

pub(crate) fn lower_awaits_in_core_blockpy_callable_def(
    callable_def: BlockPyFunction<CoreBlockPyPass>,
) -> BlockPyFunction<CoreBlockPyPassWithoutAwait> {
    BlockPyFunction {
        function_id: callable_def.function_id,
        names: callable_def.names,
        kind: callable_def.kind,
        params: callable_def.params,
        param_defaults: callable_def
            .param_defaults
            .into_iter()
            .map(lower_core_expr_awaits)
            .collect(),
        blocks: callable_def
            .blocks
            .into_iter()
            .map(lower_core_block_awaits)
            .collect(),
        doc: callable_def.doc,
        closure_layout: callable_def.closure_layout,
        facts: callable_def.facts,
        try_regions: callable_def.try_regions,
        extra: callable_def.extra,
    }
}

#[cfg(test)]
type TestCoreBlockPyModule = BlockPyModule<CoreBlockPyPass>;

#[cfg(test)]
type TestCoreBlockPyModuleWithoutAwait = BlockPyModule<CoreBlockPyPassWithoutAwait>;

#[cfg(test)]
pub(crate) fn lower_awaits_in_core_blockpy_module(
    module: TestCoreBlockPyModule,
) -> TestCoreBlockPyModuleWithoutAwait {
    BlockPyModule {
        callable_defs: module
            .callable_defs
            .into_iter()
            .map(lower_awaits_in_core_blockpy_callable_def)
            .collect(),
    }
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
