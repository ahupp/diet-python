use crate::block_py::structured::IntoStructuredBlockPyStmt;
use crate::block_py::BlockPyAssign;
use crate::block_py::{
    expr_any, BlockPyBranchTable, BlockPyCfgFragment, BlockPyDelete, BlockPyFunction, BlockPyIf,
    BlockPyIfTerm, BlockPyRaise, BlockPyTerm, CfgBlock, CoreBlockPyAwait,
    CoreBlockPyExprWithAwaitAndYield, CoreBlockPyExprWithYield, CoreBlockPyYield,
    CoreBlockPyYieldFrom, StructuredBlockPyStmt,
};
use crate::namegen::fresh_name;
use crate::passes::ruff_to_blockpy::lower_structured_blocks_to_bb_blocks;
use crate::passes::CoreBlockPyPassWithYield;
use crate::py_expr;
use ruff_python_ast as ast;

fn fresh_eval_name() -> ast::ExprName {
    let name = fresh_name("eval");
    let ast::Expr::Name(expr) = py_expr!("{name:id}", name = name.as_str()) else {
        unreachable!();
    };
    expr
}

fn expr_contains_suspend(expr: &CoreBlockPyExprWithAwaitAndYield) -> bool {
    expr_any(expr, |expr| {
        matches!(
            expr,
            CoreBlockPyExprWithAwaitAndYield::Await(_)
                | CoreBlockPyExprWithAwaitAndYield::Yield(_)
                | CoreBlockPyExprWithAwaitAndYield::YieldFrom(_)
        )
    })
}

fn hoist_core_expr_if_contains_suspend(
    expr: CoreBlockPyExprWithAwaitAndYield,
    out: &mut Vec<StructuredBlockPyStmt<CoreBlockPyExprWithAwaitAndYield>>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithAwaitAndYield {
    let expr = make_eval_order_explicit_in_core_expr(expr, out, cleanup);
    if expr_contains_suspend(&expr) {
        let target = fresh_eval_name();
        out.push(StructuredBlockPyStmt::Assign(BlockPyAssign {
            target: target.clone(),
            value: expr,
        }));
        cleanup.push(target.clone());
        CoreBlockPyExprWithAwaitAndYield::Name(target)
    } else {
        expr
    }
}

fn make_eval_order_explicit_in_core_expr(
    expr: CoreBlockPyExprWithAwaitAndYield,
    out: &mut Vec<StructuredBlockPyStmt<CoreBlockPyExprWithAwaitAndYield>>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithAwaitAndYield {
    match expr {
        CoreBlockPyExprWithAwaitAndYield::Name(_)
        | CoreBlockPyExprWithAwaitAndYield::Literal(_) => expr,
        CoreBlockPyExprWithAwaitAndYield::Op(operation) => CoreBlockPyExprWithAwaitAndYield::Op(
            operation
                .map_expr(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup)),
        ),
        CoreBlockPyExprWithAwaitAndYield::Await(await_expr) => {
            CoreBlockPyExprWithAwaitAndYield::Await(CoreBlockPyAwait {
                node_index: await_expr.node_index,
                range: await_expr.range,
                value: Box::new(hoist_core_expr_if_contains_suspend(
                    *await_expr.value,
                    out,
                    cleanup,
                )),
            })
        }
        CoreBlockPyExprWithAwaitAndYield::Yield(yield_expr) => {
            CoreBlockPyExprWithAwaitAndYield::Yield(CoreBlockPyYield {
                node_index: yield_expr.node_index,
                range: yield_expr.range,
                value: yield_expr.value.map(|value| {
                    Box::new(hoist_core_expr_if_contains_suspend(*value, out, cleanup))
                }),
            })
        }
        CoreBlockPyExprWithAwaitAndYield::YieldFrom(yield_from_expr) => {
            CoreBlockPyExprWithAwaitAndYield::YieldFrom(CoreBlockPyYieldFrom {
                node_index: yield_from_expr.node_index,
                range: yield_from_expr.range,
                value: Box::new(hoist_core_expr_if_contains_suspend(
                    *yield_from_expr.value,
                    out,
                    cleanup,
                )),
            })
        }
    }
}

fn append_stmt_cleanup<E>(out: &mut Vec<StructuredBlockPyStmt<E>>, cleanup: Vec<ast::ExprName>) {
    for temp in cleanup.into_iter().rev() {
        out.push(StructuredBlockPyStmt::Delete(BlockPyDelete {
            target: temp,
        }));
    }
}

fn make_eval_order_explicit_in_core_fragment(
    fragment: BlockPyCfgFragment<
        StructuredBlockPyStmt<CoreBlockPyExprWithAwaitAndYield>,
        BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
    >,
) -> BlockPyCfgFragment<
    StructuredBlockPyStmt<CoreBlockPyExprWithAwaitAndYield>,
    BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
> {
    let mut body = Vec::new();
    for stmt in fragment.body {
        make_eval_order_explicit_in_core_stmt(stmt, &mut body);
    }
    let term = fragment
        .term
        .map(|term| make_eval_order_explicit_in_core_term(term, &mut body));
    BlockPyCfgFragment { body, term }
}

fn make_eval_order_explicit_in_core_stmt(
    stmt: StructuredBlockPyStmt<CoreBlockPyExprWithAwaitAndYield>,
    out: &mut Vec<StructuredBlockPyStmt<CoreBlockPyExprWithAwaitAndYield>>,
) {
    match stmt {
        StructuredBlockPyStmt::Assign(assign) => {
            let mut setup = Vec::new();
            let mut cleanup = Vec::new();
            let value =
                make_eval_order_explicit_in_core_expr(assign.value, &mut setup, &mut cleanup);
            out.extend(setup);
            out.push(StructuredBlockPyStmt::Assign(BlockPyAssign {
                target: assign.target,
                value,
            }));
            append_stmt_cleanup(out, cleanup);
        }
        StructuredBlockPyStmt::Expr(expr) => {
            let mut setup = Vec::new();
            let mut cleanup = Vec::new();
            let expr = make_eval_order_explicit_in_core_expr(expr, &mut setup, &mut cleanup);
            out.extend(setup);
            out.push(StructuredBlockPyStmt::Expr(expr));
            append_stmt_cleanup(out, cleanup);
        }
        StructuredBlockPyStmt::Delete(delete) => out.push(StructuredBlockPyStmt::Delete(delete)),
        StructuredBlockPyStmt::If(if_stmt) => {
            let mut setup = Vec::new();
            let mut cleanup = Vec::new();
            let test = hoist_core_expr_if_contains_suspend(if_stmt.test, &mut setup, &mut cleanup);
            out.extend(setup);
            out.push(StructuredBlockPyStmt::If(BlockPyIf {
                test,
                body: make_eval_order_explicit_in_core_fragment(if_stmt.body),
                orelse: make_eval_order_explicit_in_core_fragment(if_stmt.orelse),
            }));
            append_stmt_cleanup(out, cleanup);
        }
    }
}

fn make_eval_order_explicit_in_core_term(
    term: BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
    out: &mut Vec<StructuredBlockPyStmt<CoreBlockPyExprWithAwaitAndYield>>,
) -> BlockPyTerm<CoreBlockPyExprWithAwaitAndYield> {
    match term {
        BlockPyTerm::Jump(edge) => BlockPyTerm::Jump(edge),
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test,
            then_label,
            else_label,
        }) => BlockPyTerm::IfTerm(BlockPyIfTerm {
            test: hoist_core_expr_if_contains_suspend(test, out, &mut Vec::new()),
            then_label,
            else_label,
        }),
        BlockPyTerm::BranchTable(BlockPyBranchTable {
            index,
            targets,
            default_label,
        }) => BlockPyTerm::BranchTable(BlockPyBranchTable {
            index: hoist_core_expr_if_contains_suspend(index, out, &mut Vec::new()),
            targets,
            default_label,
        }),
        BlockPyTerm::Raise(BlockPyRaise { exc }) => BlockPyTerm::Raise(BlockPyRaise {
            exc: exc.map(|value| hoist_core_expr_if_contains_suspend(value, out, &mut Vec::new())),
        }),
        BlockPyTerm::Return(value) => BlockPyTerm::Return(hoist_core_expr_if_contains_suspend(
            value,
            out,
            &mut Vec::new(),
        )),
    }
}

pub(crate) fn make_eval_order_explicit_in_core_block(
    block: CfgBlock<
        StructuredBlockPyStmt<CoreBlockPyExprWithAwaitAndYield>,
        BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
    >,
) -> CfgBlock<
    StructuredBlockPyStmt<CoreBlockPyExprWithAwaitAndYield>,
    BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
> {
    let CfgBlock {
        label,
        body: input_body,
        term: input_term,
        params,
        exc_edge,
    } = block;
    let mut body = Vec::new();
    for stmt in input_body {
        make_eval_order_explicit_in_core_stmt(stmt, &mut body);
    }
    let term = make_eval_order_explicit_in_core_term(input_term, &mut body);
    CfgBlock {
        label,
        body,
        term,
        params,
        exc_edge,
    }
}

fn is_core_atom_without_await(expr: &CoreBlockPyExprWithYield) -> bool {
    matches!(
        expr,
        CoreBlockPyExprWithYield::Name(_) | CoreBlockPyExprWithYield::Literal(_)
    ) || matches!(
        expr,
        CoreBlockPyExprWithYield::Op(operation)
            if matches!(
                operation.detail(),
                crate::block_py::OperationDetail::LoadName(_)
                    | crate::block_py::OperationDetail::LoadRuntime(_)
            )
    )
}

fn expr_contains_yield(expr: &CoreBlockPyExprWithYield) -> bool {
    expr_any(expr, |expr| {
        matches!(
            expr,
            CoreBlockPyExprWithYield::Yield(_) | CoreBlockPyExprWithYield::YieldFrom(_)
        )
    })
}

fn hoist_core_expr_without_await_to_atom(
    expr: CoreBlockPyExprWithYield,
    out: &mut Vec<StructuredBlockPyStmt<CoreBlockPyExprWithYield>>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithYield {
    let expr = make_eval_order_explicit_in_core_expr_without_await(expr, out, cleanup);
    if is_core_atom_without_await(&expr) {
        expr
    } else {
        let target = fresh_eval_name();
        out.push(StructuredBlockPyStmt::Assign(BlockPyAssign {
            target: target.clone(),
            value: expr,
        }));
        cleanup.push(target.clone());
        CoreBlockPyExprWithYield::Name(target)
    }
}

fn make_eval_order_explicit_in_core_expr_without_await(
    expr: CoreBlockPyExprWithYield,
    out: &mut Vec<StructuredBlockPyStmt<CoreBlockPyExprWithYield>>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithYield {
    match expr {
        CoreBlockPyExprWithYield::Name(_) | CoreBlockPyExprWithYield::Literal(_) => expr,
        CoreBlockPyExprWithYield::Op(operation) => CoreBlockPyExprWithYield::Op(
            operation
                .map_expr(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup)),
        ),
        CoreBlockPyExprWithYield::Yield(yield_expr) => {
            CoreBlockPyExprWithYield::Yield(CoreBlockPyYield {
                node_index: yield_expr.node_index,
                range: yield_expr.range,
                value: yield_expr.value.map(|value| {
                    Box::new(hoist_core_expr_without_await_to_atom(*value, out, cleanup))
                }),
            })
        }
        CoreBlockPyExprWithYield::YieldFrom(yield_from_expr) => {
            CoreBlockPyExprWithYield::YieldFrom(CoreBlockPyYieldFrom {
                node_index: yield_from_expr.node_index,
                range: yield_from_expr.range,
                value: Box::new(hoist_core_expr_without_await_to_atom(
                    *yield_from_expr.value,
                    out,
                    cleanup,
                )),
            })
        }
    }
}

fn make_eval_order_explicit_in_core_stmt_without_await(
    stmt: StructuredBlockPyStmt<CoreBlockPyExprWithYield>,
    out: &mut Vec<StructuredBlockPyStmt<CoreBlockPyExprWithYield>>,
) {
    match stmt {
        StructuredBlockPyStmt::Assign(assign) => {
            let mut setup = Vec::new();
            let mut cleanup = Vec::new();
            let value = if expr_contains_yield(&assign.value) {
                make_eval_order_explicit_in_core_expr_without_await(
                    assign.value,
                    &mut setup,
                    &mut cleanup,
                )
            } else {
                assign.value
            };
            out.extend(setup);
            out.push(StructuredBlockPyStmt::Assign(BlockPyAssign {
                target: assign.target,
                value,
            }));
            append_stmt_cleanup(out, cleanup);
        }
        StructuredBlockPyStmt::Expr(expr) => {
            let mut setup = Vec::new();
            let mut cleanup = Vec::new();
            let expr = if expr_contains_yield(&expr) {
                make_eval_order_explicit_in_core_expr_without_await(expr, &mut setup, &mut cleanup)
            } else {
                expr
            };
            out.extend(setup);
            out.push(StructuredBlockPyStmt::Expr(expr));
            append_stmt_cleanup(out, cleanup);
        }
        StructuredBlockPyStmt::Delete(delete) => out.push(StructuredBlockPyStmt::Delete(delete)),
        StructuredBlockPyStmt::If(if_stmt) => {
            let mut setup = Vec::new();
            let mut cleanup = Vec::new();
            let test =
                hoist_core_expr_without_await_to_atom(if_stmt.test, &mut setup, &mut cleanup);
            out.extend(setup);
            out.push(StructuredBlockPyStmt::If(BlockPyIf {
                test,
                body: make_eval_order_explicit_in_core_fragment_without_await(if_stmt.body),
                orelse: make_eval_order_explicit_in_core_fragment_without_await(if_stmt.orelse),
            }));
            append_stmt_cleanup(out, cleanup);
        }
    }
}

fn make_eval_order_explicit_in_core_fragment_without_await(
    fragment: BlockPyCfgFragment<
        StructuredBlockPyStmt<CoreBlockPyExprWithYield>,
        BlockPyTerm<CoreBlockPyExprWithYield>,
    >,
) -> BlockPyCfgFragment<
    StructuredBlockPyStmt<CoreBlockPyExprWithYield>,
    BlockPyTerm<CoreBlockPyExprWithYield>,
> {
    let mut body = Vec::new();
    for stmt in fragment.body {
        make_eval_order_explicit_in_core_stmt_without_await(stmt, &mut body);
    }
    let term = fragment
        .term
        .map(|term| make_eval_order_explicit_in_core_term_without_await(term, &mut body));
    BlockPyCfgFragment { body, term }
}

fn make_eval_order_explicit_in_core_term_without_await(
    term: BlockPyTerm<CoreBlockPyExprWithYield>,
    out: &mut Vec<StructuredBlockPyStmt<CoreBlockPyExprWithYield>>,
) -> BlockPyTerm<CoreBlockPyExprWithYield> {
    match term {
        BlockPyTerm::Jump(_) => term,
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test,
            then_label,
            else_label,
        }) => BlockPyTerm::IfTerm(BlockPyIfTerm {
            test: hoist_core_expr_without_await_to_atom(test, out, &mut Vec::new()),
            then_label,
            else_label,
        }),
        BlockPyTerm::BranchTable(BlockPyBranchTable {
            index,
            targets,
            default_label,
        }) => BlockPyTerm::BranchTable(BlockPyBranchTable {
            index: hoist_core_expr_without_await_to_atom(index, out, &mut Vec::new()),
            targets,
            default_label,
        }),
        BlockPyTerm::Raise(BlockPyRaise { exc }) => BlockPyTerm::Raise(BlockPyRaise {
            exc: exc
                .map(|value| hoist_core_expr_without_await_to_atom(value, out, &mut Vec::new())),
        }),
        BlockPyTerm::Return(value) => BlockPyTerm::Return(hoist_core_expr_without_await_to_atom(
            value,
            out,
            &mut Vec::new(),
        )),
    }
}

pub(crate) fn make_eval_order_explicit_in_core_block_without_await(
    block: CfgBlock<
        StructuredBlockPyStmt<CoreBlockPyExprWithYield>,
        BlockPyTerm<CoreBlockPyExprWithYield>,
    >,
) -> CfgBlock<StructuredBlockPyStmt<CoreBlockPyExprWithYield>, BlockPyTerm<CoreBlockPyExprWithYield>>
{
    let CfgBlock {
        label,
        body: input_body,
        term: input_term,
        params,
        exc_edge,
    } = block;
    let mut body = Vec::new();
    for stmt in input_body {
        make_eval_order_explicit_in_core_stmt_without_await(stmt, &mut body);
    }
    let term = make_eval_order_explicit_in_core_term_without_await(input_term, &mut body);
    CfgBlock {
        label,
        body,
        term,
        params,
        exc_edge,
    }
}

pub(crate) fn make_eval_order_explicit_in_core_callable_def_without_await(
    callable_def: BlockPyFunction<CoreBlockPyPassWithYield>,
) -> BlockPyFunction<CoreBlockPyPassWithYield> {
    let BlockPyFunction {
        function_id,
        name_gen,
        names,
        kind,
        params,
        blocks,
        doc,
        storage_layout,
        semantic,
    } = callable_def;
    let structured_blocks = blocks
        .into_iter()
        .map(|block| CfgBlock {
            label: block.label,
            body: block
                .body
                .into_iter()
                .map(|stmt| stmt.into_structured_stmt())
                .collect(),
            term: block.term,
            params: block.params,
            exc_edge: block.exc_edge,
        })
        .map(make_eval_order_explicit_in_core_block_without_await)
        .collect::<Vec<_>>();
    BlockPyFunction {
        function_id,
        name_gen,
        names,
        kind,
        params,
        blocks: lower_structured_blocks_to_bb_blocks(&structured_blocks),
        doc,
        storage_layout,
        semantic,
    }
}

#[cfg(test)]
mod test;
