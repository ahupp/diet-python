use crate::block_py::BlockPyAssign;
use crate::block_py::{
    BlockPyBranchTable, BlockPyCfgFragment, BlockPyDelete, BlockPyFunction, BlockPyIf,
    BlockPyIfTerm, BlockPyRaise, BlockPyStmt, BlockPyTerm, CfgBlock, CoreBlockPyAwait,
    CoreBlockPyCall, CoreBlockPyCallArg, CoreBlockPyExprWithAwaitAndYield,
    CoreBlockPyExprWithYield, CoreBlockPyKeywordArg, CoreBlockPyYield, CoreBlockPyYieldFrom,
    IntrinsicCall,
};
use crate::namegen::fresh_name;
use crate::passes::CoreBlockPyPassWithoutAwait;
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
    match expr {
        CoreBlockPyExprWithAwaitAndYield::Name(_)
        | CoreBlockPyExprWithAwaitAndYield::Literal(_) => false,
        CoreBlockPyExprWithAwaitAndYield::Call(call) => {
            expr_contains_suspend(&call.func)
                || call.args.iter().any(|arg| match arg {
                    CoreBlockPyCallArg::Positional(value) | CoreBlockPyCallArg::Starred(value) => {
                        expr_contains_suspend(value)
                    }
                })
                || call.keywords.iter().any(|keyword| match keyword {
                    CoreBlockPyKeywordArg::Named { value, .. }
                    | CoreBlockPyKeywordArg::Starred(value) => expr_contains_suspend(value),
                })
        }
        CoreBlockPyExprWithAwaitAndYield::Intrinsic(call) => {
            call.args.iter().any(|arg| match arg {
                CoreBlockPyCallArg::Positional(value) | CoreBlockPyCallArg::Starred(value) => {
                    expr_contains_suspend(value)
                }
            }) || call.keywords.iter().any(|keyword| match keyword {
                CoreBlockPyKeywordArg::Named { value, .. }
                | CoreBlockPyKeywordArg::Starred(value) => expr_contains_suspend(value),
            })
        }
        CoreBlockPyExprWithAwaitAndYield::Await(_) => true,
        CoreBlockPyExprWithAwaitAndYield::Yield(_) => true,
        CoreBlockPyExprWithAwaitAndYield::YieldFrom(_) => true,
    }
}

fn hoist_core_expr_if_contains_suspend(
    expr: CoreBlockPyExprWithAwaitAndYield,
    out: &mut Vec<BlockPyStmt<CoreBlockPyExprWithAwaitAndYield>>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithAwaitAndYield {
    let expr = make_eval_order_explicit_in_core_expr(expr, out, cleanup);
    if expr_contains_suspend(&expr) {
        let target = fresh_eval_name();
        out.push(BlockPyStmt::Assign(BlockPyAssign {
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
    out: &mut Vec<BlockPyStmt<CoreBlockPyExprWithAwaitAndYield>>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithAwaitAndYield {
    match expr {
        CoreBlockPyExprWithAwaitAndYield::Name(_)
        | CoreBlockPyExprWithAwaitAndYield::Literal(_) => expr,
        CoreBlockPyExprWithAwaitAndYield::Call(call) => {
            CoreBlockPyExprWithAwaitAndYield::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(hoist_core_expr_if_contains_suspend(
                    *call.func, out, cleanup,
                )),
                args: call
                    .args
                    .into_iter()
                    .map(|arg| match arg {
                        CoreBlockPyCallArg::Positional(value) => CoreBlockPyCallArg::Positional(
                            hoist_core_expr_if_contains_suspend(value, out, cleanup),
                        ),
                        CoreBlockPyCallArg::Starred(value) => CoreBlockPyCallArg::Starred(
                            hoist_core_expr_if_contains_suspend(value, out, cleanup),
                        ),
                    })
                    .collect(),
                keywords: call
                    .keywords
                    .into_iter()
                    .map(|keyword| match keyword {
                        CoreBlockPyKeywordArg::Named { arg, value } => {
                            CoreBlockPyKeywordArg::Named {
                                arg,
                                value: hoist_core_expr_if_contains_suspend(value, out, cleanup),
                            }
                        }
                        CoreBlockPyKeywordArg::Starred(value) => CoreBlockPyKeywordArg::Starred(
                            hoist_core_expr_if_contains_suspend(value, out, cleanup),
                        ),
                    })
                    .collect(),
            })
        }
        CoreBlockPyExprWithAwaitAndYield::Intrinsic(call) => {
            CoreBlockPyExprWithAwaitAndYield::Intrinsic(IntrinsicCall {
                intrinsic: call.intrinsic,
                node_index: call.node_index,
                range: call.range,
                args: call
                    .args
                    .into_iter()
                    .map(|arg| match arg {
                        CoreBlockPyCallArg::Positional(value) => CoreBlockPyCallArg::Positional(
                            hoist_core_expr_if_contains_suspend(value, out, cleanup),
                        ),
                        CoreBlockPyCallArg::Starred(value) => CoreBlockPyCallArg::Starred(
                            hoist_core_expr_if_contains_suspend(value, out, cleanup),
                        ),
                    })
                    .collect(),
                keywords: call
                    .keywords
                    .into_iter()
                    .map(|keyword| match keyword {
                        CoreBlockPyKeywordArg::Named { arg, value } => {
                            CoreBlockPyKeywordArg::Named {
                                arg,
                                value: hoist_core_expr_if_contains_suspend(value, out, cleanup),
                            }
                        }
                        CoreBlockPyKeywordArg::Starred(value) => CoreBlockPyKeywordArg::Starred(
                            hoist_core_expr_if_contains_suspend(value, out, cleanup),
                        ),
                    })
                    .collect(),
            })
        }
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

fn append_stmt_cleanup<E>(out: &mut Vec<BlockPyStmt<E>>, cleanup: Vec<ast::ExprName>) {
    for temp in cleanup.into_iter().rev() {
        out.push(BlockPyStmt::Delete(BlockPyDelete { target: temp }));
    }
}

fn make_eval_order_explicit_in_core_fragment(
    fragment: BlockPyCfgFragment<
        BlockPyStmt<CoreBlockPyExprWithAwaitAndYield>,
        BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
    >,
) -> BlockPyCfgFragment<
    BlockPyStmt<CoreBlockPyExprWithAwaitAndYield>,
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
    stmt: BlockPyStmt<CoreBlockPyExprWithAwaitAndYield>,
    out: &mut Vec<BlockPyStmt<CoreBlockPyExprWithAwaitAndYield>>,
) {
    match stmt {
        BlockPyStmt::Assign(assign) => {
            let mut setup = Vec::new();
            let mut cleanup = Vec::new();
            let value =
                make_eval_order_explicit_in_core_expr(assign.value, &mut setup, &mut cleanup);
            out.extend(setup);
            out.push(BlockPyStmt::Assign(BlockPyAssign {
                target: assign.target,
                value,
            }));
            append_stmt_cleanup(out, cleanup);
        }
        BlockPyStmt::Expr(expr) => {
            let mut setup = Vec::new();
            let mut cleanup = Vec::new();
            let expr = make_eval_order_explicit_in_core_expr(expr, &mut setup, &mut cleanup);
            out.extend(setup);
            out.push(BlockPyStmt::Expr(expr));
            append_stmt_cleanup(out, cleanup);
        }
        BlockPyStmt::Delete(delete) => out.push(BlockPyStmt::Delete(delete)),
        BlockPyStmt::If(if_stmt) => {
            let mut setup = Vec::new();
            let mut cleanup = Vec::new();
            let test = hoist_core_expr_if_contains_suspend(if_stmt.test, &mut setup, &mut cleanup);
            out.extend(setup);
            out.push(BlockPyStmt::If(BlockPyIf {
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
    out: &mut Vec<BlockPyStmt<CoreBlockPyExprWithAwaitAndYield>>,
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
        BlockPyStmt<CoreBlockPyExprWithAwaitAndYield>,
        BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
    >,
) -> CfgBlock<
    BlockPyStmt<CoreBlockPyExprWithAwaitAndYield>,
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
    )
}

fn expr_contains_yield(expr: &CoreBlockPyExprWithYield) -> bool {
    match expr {
        CoreBlockPyExprWithYield::Name(_) | CoreBlockPyExprWithYield::Literal(_) => false,
        CoreBlockPyExprWithYield::Call(call) => {
            expr_contains_yield(&call.func)
                || call.args.iter().any(|arg| match arg {
                    CoreBlockPyCallArg::Positional(value) | CoreBlockPyCallArg::Starred(value) => {
                        expr_contains_yield(value)
                    }
                })
                || call.keywords.iter().any(|keyword| match keyword {
                    CoreBlockPyKeywordArg::Named { value, .. }
                    | CoreBlockPyKeywordArg::Starred(value) => expr_contains_yield(value),
                })
        }
        CoreBlockPyExprWithYield::Intrinsic(call) => {
            call.args.iter().any(|arg| match arg {
                CoreBlockPyCallArg::Positional(value) | CoreBlockPyCallArg::Starred(value) => {
                    expr_contains_yield(value)
                }
            }) || call.keywords.iter().any(|keyword| match keyword {
                CoreBlockPyKeywordArg::Named { value, .. }
                | CoreBlockPyKeywordArg::Starred(value) => expr_contains_yield(value),
            })
        }
        CoreBlockPyExprWithYield::Yield(_) => true,
        CoreBlockPyExprWithYield::YieldFrom(_) => true,
    }
}

fn hoist_core_expr_without_await_to_atom(
    expr: CoreBlockPyExprWithYield,
    out: &mut Vec<BlockPyStmt<CoreBlockPyExprWithYield>>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithYield {
    let expr = make_eval_order_explicit_in_core_expr_without_await(expr, out, cleanup);
    if is_core_atom_without_await(&expr) {
        expr
    } else {
        let target = fresh_eval_name();
        out.push(BlockPyStmt::Assign(BlockPyAssign {
            target: target.clone(),
            value: expr,
        }));
        cleanup.push(target.clone());
        CoreBlockPyExprWithYield::Name(target)
    }
}

fn make_eval_order_explicit_in_core_expr_without_await(
    expr: CoreBlockPyExprWithYield,
    out: &mut Vec<BlockPyStmt<CoreBlockPyExprWithYield>>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithYield {
    match expr {
        CoreBlockPyExprWithYield::Name(_) | CoreBlockPyExprWithYield::Literal(_) => expr,
        CoreBlockPyExprWithYield::Call(call) => CoreBlockPyExprWithYield::Call(CoreBlockPyCall {
            node_index: call.node_index,
            range: call.range,
            func: Box::new(hoist_core_expr_without_await_to_atom(
                *call.func, out, cleanup,
            )),
            args: call
                .args
                .into_iter()
                .map(|arg| match arg {
                    CoreBlockPyCallArg::Positional(value) => CoreBlockPyCallArg::Positional(
                        hoist_core_expr_without_await_to_atom(value, out, cleanup),
                    ),
                    CoreBlockPyCallArg::Starred(value) => CoreBlockPyCallArg::Starred(
                        hoist_core_expr_without_await_to_atom(value, out, cleanup),
                    ),
                })
                .collect(),
            keywords: call
                .keywords
                .into_iter()
                .map(|keyword| match keyword {
                    CoreBlockPyKeywordArg::Named { arg, value } => CoreBlockPyKeywordArg::Named {
                        arg,
                        value: hoist_core_expr_without_await_to_atom(value, out, cleanup),
                    },
                    CoreBlockPyKeywordArg::Starred(value) => CoreBlockPyKeywordArg::Starred(
                        hoist_core_expr_without_await_to_atom(value, out, cleanup),
                    ),
                })
                .collect(),
        }),
        CoreBlockPyExprWithYield::Intrinsic(call) => {
            CoreBlockPyExprWithYield::Intrinsic(IntrinsicCall {
                intrinsic: call.intrinsic,
                node_index: call.node_index,
                range: call.range,
                args: call
                    .args
                    .into_iter()
                    .map(|arg| match arg {
                        CoreBlockPyCallArg::Positional(value) => CoreBlockPyCallArg::Positional(
                            hoist_core_expr_without_await_to_atom(value, out, cleanup),
                        ),
                        CoreBlockPyCallArg::Starred(value) => CoreBlockPyCallArg::Starred(
                            hoist_core_expr_without_await_to_atom(value, out, cleanup),
                        ),
                    })
                    .collect(),
                keywords: call
                    .keywords
                    .into_iter()
                    .map(|keyword| match keyword {
                        CoreBlockPyKeywordArg::Named { arg, value } => {
                            CoreBlockPyKeywordArg::Named {
                                arg,
                                value: hoist_core_expr_without_await_to_atom(value, out, cleanup),
                            }
                        }
                        CoreBlockPyKeywordArg::Starred(value) => CoreBlockPyKeywordArg::Starred(
                            hoist_core_expr_without_await_to_atom(value, out, cleanup),
                        ),
                    })
                    .collect(),
            })
        }
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
    stmt: BlockPyStmt<CoreBlockPyExprWithYield>,
    out: &mut Vec<BlockPyStmt<CoreBlockPyExprWithYield>>,
) {
    match stmt {
        BlockPyStmt::Assign(assign) => {
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
            out.push(BlockPyStmt::Assign(BlockPyAssign {
                target: assign.target,
                value,
            }));
            append_stmt_cleanup(out, cleanup);
        }
        BlockPyStmt::Expr(expr) => {
            let mut setup = Vec::new();
            let mut cleanup = Vec::new();
            let expr = if expr_contains_yield(&expr) {
                make_eval_order_explicit_in_core_expr_without_await(expr, &mut setup, &mut cleanup)
            } else {
                expr
            };
            out.extend(setup);
            out.push(BlockPyStmt::Expr(expr));
            append_stmt_cleanup(out, cleanup);
        }
        BlockPyStmt::Delete(delete) => out.push(BlockPyStmt::Delete(delete)),
        BlockPyStmt::If(if_stmt) => {
            let mut setup = Vec::new();
            let mut cleanup = Vec::new();
            let test =
                hoist_core_expr_without_await_to_atom(if_stmt.test, &mut setup, &mut cleanup);
            out.extend(setup);
            out.push(BlockPyStmt::If(BlockPyIf {
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
        BlockPyStmt<CoreBlockPyExprWithYield>,
        BlockPyTerm<CoreBlockPyExprWithYield>,
    >,
) -> BlockPyCfgFragment<BlockPyStmt<CoreBlockPyExprWithYield>, BlockPyTerm<CoreBlockPyExprWithYield>>
{
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
    out: &mut Vec<BlockPyStmt<CoreBlockPyExprWithYield>>,
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
    block: CfgBlock<BlockPyStmt<CoreBlockPyExprWithYield>, BlockPyTerm<CoreBlockPyExprWithYield>>,
) -> CfgBlock<BlockPyStmt<CoreBlockPyExprWithYield>, BlockPyTerm<CoreBlockPyExprWithYield>> {
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
    callable_def: BlockPyFunction<CoreBlockPyPassWithoutAwait>,
) -> BlockPyFunction<CoreBlockPyPassWithoutAwait> {
    callable_def.map_blocks(make_eval_order_explicit_in_core_block_without_await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_py::{
        BlockPyBlock, BlockPyLabel, BlockPyTerm, CoreBlockPyExprWithAwaitAndYield,
    };

    fn test_name(id: &str) -> ast::ExprName {
        let ast::Expr::Name(expr) = crate::py_expr!("{id:id}", id = id) else {
            unreachable!();
        };
        expr
    }

    #[test]
    fn eval_order_hoists_call_arguments_in_return_value_to_temps() {
        let block = BlockPyBlock {
            label: BlockPyLabel("start".to_string()),
            body: Vec::new(),
            term: BlockPyTerm::Return(CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!(
                "f(g(x), h(y))"
            ))),
            params: Vec::new(),
            exc_edge: None,
        };

        let lowered = make_eval_order_explicit_in_core_block(block);
        assert!(lowered.body.is_empty());
        let BlockPyTerm::Return(CoreBlockPyExprWithAwaitAndYield::Call(call)) = &lowered.term
        else {
            panic!("expected call expr");
        };
        assert!(matches!(
            call.func.as_ref(),
            CoreBlockPyExprWithAwaitAndYield::Name(_)
        ));
        assert!(matches!(
            &call.args[0],
            CoreBlockPyCallArg::Positional(CoreBlockPyExprWithAwaitAndYield::Call(_))
        ));
        assert!(matches!(
            &call.args[1],
            CoreBlockPyCallArg::Positional(CoreBlockPyExprWithAwaitAndYield::Call(_))
        ));
    }

    #[test]
    fn eval_order_hoists_return_value_to_temp() {
        let block = BlockPyBlock {
            label: BlockPyLabel("start".to_string()),
            body: Vec::new(),
            term: BlockPyTerm::Return(CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!(
                "f(g(x))"
            ))),
            params: Vec::new(),
            exc_edge: None,
        };

        let lowered = make_eval_order_explicit_in_core_block(block);
        assert!(lowered.body.is_empty());
        let BlockPyTerm::Return(CoreBlockPyExprWithAwaitAndYield::Call(call)) = lowered.term else {
            panic!("expected return of recursive call");
        };
        assert!(matches!(
            call.func.as_ref(),
            CoreBlockPyExprWithAwaitAndYield::Name(_)
        ));
    }

    #[test]
    fn eval_order_hoists_nested_call_in_assignment_rhs() {
        let block = BlockPyBlock {
            label: BlockPyLabel("start".to_string()),
            body: vec![BlockPyStmt::Assign(BlockPyAssign {
                target: fresh_eval_name(),
                value: CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!("f(g(x))")),
            })],
            term: BlockPyTerm::Return(CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!(
                "__dp_NONE"
            ))),
            params: Vec::new(),
            exc_edge: None,
        };

        let lowered = make_eval_order_explicit_in_core_block(block);
        assert_eq!(lowered.body.len(), 1);
        let BlockPyStmt::Assign(assign) = &lowered.body[0] else {
            panic!("expected rewritten assignment");
        };
        let CoreBlockPyExprWithAwaitAndYield::Call(call) = &assign.value else {
            panic!("expected outer call");
        };
        assert!(matches!(
            call.func.as_ref(),
            CoreBlockPyExprWithAwaitAndYield::Name(_)
        ));
        assert!(matches!(
            &call.args[0],
            CoreBlockPyCallArg::Positional(CoreBlockPyExprWithAwaitAndYield::Call(_))
        ));
    }

    #[test]
    fn eval_order_hoists_await_in_assignment_call_argument() {
        let block = BlockPyBlock {
            label: BlockPyLabel("start".to_string()),
            body: vec![BlockPyStmt::Assign(BlockPyAssign {
                target: test_name("total"),
                value: CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!(
                    "__dp_iadd(total, await Once())"
                )),
            })],
            term: BlockPyTerm::Return(CoreBlockPyExprWithAwaitAndYield::from(crate::py_expr!(
                "__dp_NONE"
            ))),
            params: Vec::new(),
            exc_edge: None,
        };

        let lowered = make_eval_order_explicit_in_core_block(block);
        assert_eq!(lowered.body.len(), 3);
        let BlockPyStmt::Assign(temp_assign) = &lowered.body[0] else {
            panic!("expected hoisted await temp assignment");
        };
        assert!(matches!(
            temp_assign.value,
            CoreBlockPyExprWithAwaitAndYield::Await(_)
        ));
        let BlockPyStmt::Assign(assign) = &lowered.body[1] else {
            panic!("expected rewritten assignment");
        };
        let CoreBlockPyExprWithAwaitAndYield::Call(call) = &assign.value else {
            panic!("expected iadd call");
        };
        assert!(matches!(
            &call.args[1],
            CoreBlockPyCallArg::Positional(CoreBlockPyExprWithAwaitAndYield::Name(_))
        ));
        assert!(matches!(lowered.body[2], BlockPyStmt::Delete(_)));
    }

    #[test]
    fn eval_order_without_await_hoists_yield_from_in_assignment_call_argument() {
        let block = BlockPyBlock {
            label: BlockPyLabel("start".to_string()),
            body: vec![BlockPyStmt::Assign(BlockPyAssign {
                target: test_name("total"),
                value: CoreBlockPyExprWithYield::Call(CoreBlockPyCall {
                    node_index: Default::default(),
                    range: Default::default(),
                    func: Box::new(CoreBlockPyExprWithYield::Name(test_name("__dp_iadd"))),
                    args: vec![
                        CoreBlockPyCallArg::Positional(CoreBlockPyExprWithYield::Name(test_name(
                            "total",
                        ))),
                        CoreBlockPyCallArg::Positional(CoreBlockPyExprWithYield::YieldFrom(
                            CoreBlockPyYieldFrom {
                                node_index: Default::default(),
                                range: Default::default(),
                                value: Box::new(CoreBlockPyExprWithYield::Name(test_name("it"))),
                            },
                        )),
                    ],
                    keywords: Vec::new(),
                }),
            })],
            term: BlockPyTerm::Return(CoreBlockPyExprWithYield::Name(test_name("__dp_NONE"))),
            params: Vec::new(),
            exc_edge: None,
        };

        let lowered = make_eval_order_explicit_in_core_block_without_await(block);
        assert_eq!(lowered.body.len(), 3);
        let BlockPyStmt::Assign(temp_assign) = &lowered.body[0] else {
            panic!("expected hoisted yield-from temp assignment");
        };
        assert!(matches!(
            temp_assign.value,
            CoreBlockPyExprWithYield::YieldFrom(_)
        ));
        let BlockPyStmt::Assign(assign) = &lowered.body[1] else {
            panic!("expected rewritten assignment");
        };
        let CoreBlockPyExprWithYield::Call(call) = &assign.value else {
            panic!("expected iadd call");
        };
        assert!(matches!(
            call.args[1],
            CoreBlockPyCallArg::Positional(CoreBlockPyExprWithYield::Name(_))
        ));
        assert!(matches!(lowered.body[2], BlockPyStmt::Delete(_)));
    }
}
