use crate::block_py::BlockPyAssign;
use crate::block_py::{
    BlockPyBranchTable, BlockPyCfgFragment, BlockPyFunction, BlockPyIf, BlockPyIfTerm,
    BlockPyRaise, BlockPyStmt, BlockPyTerm, CfgBlock, CoreBlockPyAwait, CoreBlockPyCall,
    CoreBlockPyCallArg, CoreBlockPyExpr, CoreBlockPyExprWithoutAwait, CoreBlockPyKeywordArg,
    CoreBlockPyYield, CoreBlockPyYieldFrom,
};
use crate::namegen::fresh_name;
use crate::passes::{CoreBlockPyPass, CoreBlockPyPassWithoutAwait};
use crate::py_expr;
use ruff_python_ast as ast;

fn fresh_eval_name() -> ast::ExprName {
    let name = fresh_name("eval");
    let ast::Expr::Name(expr) = py_expr!("{name:id}", name = name.as_str()) else {
        unreachable!();
    };
    expr
}

fn is_core_atom(expr: &CoreBlockPyExpr) -> bool {
    matches!(expr, CoreBlockPyExpr::Name(_) | CoreBlockPyExpr::Literal(_))
}

fn expr_contains_await_or_yield(expr: &CoreBlockPyExpr) -> bool {
    match expr {
        CoreBlockPyExpr::Name(_) | CoreBlockPyExpr::Literal(_) => false,
        CoreBlockPyExpr::Call(call) => {
            expr_contains_await_or_yield(&call.func)
                || call.args.iter().any(|arg| match arg {
                    CoreBlockPyCallArg::Positional(value) | CoreBlockPyCallArg::Starred(value) => {
                        expr_contains_await_or_yield(value)
                    }
                })
                || call.keywords.iter().any(|keyword| match keyword {
                    CoreBlockPyKeywordArg::Named { value, .. }
                    | CoreBlockPyKeywordArg::Starred(value) => expr_contains_await_or_yield(value),
                })
        }
        CoreBlockPyExpr::Await(_) => true,
        CoreBlockPyExpr::Yield(_) => true,
        CoreBlockPyExpr::YieldFrom(_) => true,
    }
}

fn hoist_core_expr_to_atom(
    expr: CoreBlockPyExpr,
    out: &mut Vec<BlockPyStmt<CoreBlockPyExpr>>,
) -> CoreBlockPyExpr {
    let expr = make_eval_order_explicit_in_core_expr(expr, out);
    if is_core_atom(&expr) {
        expr
    } else {
        let target = fresh_eval_name();
        out.push(BlockPyStmt::Assign(BlockPyAssign {
            target: target.clone(),
            value: expr,
        }));
        CoreBlockPyExpr::Name(target)
    }
}

fn make_eval_order_explicit_in_core_expr(
    expr: CoreBlockPyExpr,
    out: &mut Vec<BlockPyStmt<CoreBlockPyExpr>>,
) -> CoreBlockPyExpr {
    match expr {
        CoreBlockPyExpr::Name(_) | CoreBlockPyExpr::Literal(_) => expr,
        CoreBlockPyExpr::Call(call) => CoreBlockPyExpr::Call(CoreBlockPyCall {
            node_index: call.node_index,
            range: call.range,
            func: Box::new(hoist_core_expr_to_atom(*call.func, out)),
            args: call
                .args
                .into_iter()
                .map(|arg| match arg {
                    CoreBlockPyCallArg::Positional(value) => {
                        CoreBlockPyCallArg::Positional(hoist_core_expr_to_atom(value, out))
                    }
                    CoreBlockPyCallArg::Starred(value) => {
                        CoreBlockPyCallArg::Starred(hoist_core_expr_to_atom(value, out))
                    }
                })
                .collect(),
            keywords: call
                .keywords
                .into_iter()
                .map(|keyword| match keyword {
                    CoreBlockPyKeywordArg::Named { arg, value } => CoreBlockPyKeywordArg::Named {
                        arg,
                        value: hoist_core_expr_to_atom(value, out),
                    },
                    CoreBlockPyKeywordArg::Starred(value) => {
                        CoreBlockPyKeywordArg::Starred(hoist_core_expr_to_atom(value, out))
                    }
                })
                .collect(),
        }),
        CoreBlockPyExpr::Await(await_expr) => CoreBlockPyExpr::Await(CoreBlockPyAwait {
            node_index: await_expr.node_index,
            range: await_expr.range,
            value: Box::new(hoist_core_expr_to_atom(*await_expr.value, out)),
        }),
        CoreBlockPyExpr::Yield(yield_expr) => CoreBlockPyExpr::Yield(CoreBlockPyYield {
            node_index: yield_expr.node_index,
            range: yield_expr.range,
            value: yield_expr
                .value
                .map(|value| Box::new(hoist_core_expr_to_atom(*value, out))),
        }),
        CoreBlockPyExpr::YieldFrom(yield_from_expr) => {
            CoreBlockPyExpr::YieldFrom(CoreBlockPyYieldFrom {
                node_index: yield_from_expr.node_index,
                range: yield_from_expr.range,
                value: Box::new(hoist_core_expr_to_atom(*yield_from_expr.value, out)),
            })
        }
    }
}

fn make_eval_order_explicit_in_core_fragment(
    fragment: BlockPyCfgFragment<BlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>>,
) -> BlockPyCfgFragment<BlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>> {
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
    stmt: BlockPyStmt<CoreBlockPyExpr>,
    out: &mut Vec<BlockPyStmt<CoreBlockPyExpr>>,
) {
    match stmt {
        BlockPyStmt::Assign(assign) => {
            let value = if expr_contains_await_or_yield(&assign.value) {
                make_eval_order_explicit_in_core_expr(assign.value, out)
            } else {
                assign.value
            };
            out.push(BlockPyStmt::Assign(BlockPyAssign {
                target: assign.target,
                value,
            }));
        }
        BlockPyStmt::Expr(expr) => {
            let expr = if expr_contains_await_or_yield(&expr) {
                make_eval_order_explicit_in_core_expr(expr, out)
            } else {
                expr
            };
            out.push(BlockPyStmt::Expr(expr));
        }
        BlockPyStmt::Delete(delete) => out.push(BlockPyStmt::Delete(delete)),
        BlockPyStmt::If(if_stmt) => {
            let test = hoist_core_expr_to_atom(if_stmt.test, out);
            out.push(BlockPyStmt::If(BlockPyIf {
                test,
                body: make_eval_order_explicit_in_core_fragment(if_stmt.body),
                orelse: make_eval_order_explicit_in_core_fragment(if_stmt.orelse),
            }));
        }
    }
}

fn make_eval_order_explicit_in_core_term(
    term: BlockPyTerm<CoreBlockPyExpr>,
    out: &mut Vec<BlockPyStmt<CoreBlockPyExpr>>,
) -> BlockPyTerm<CoreBlockPyExpr> {
    match term {
        BlockPyTerm::Jump(_) | BlockPyTerm::TryJump(_) => term,
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test,
            then_label,
            else_label,
        }) => BlockPyTerm::IfTerm(BlockPyIfTerm {
            test: hoist_core_expr_to_atom(test, out),
            then_label,
            else_label,
        }),
        BlockPyTerm::BranchTable(BlockPyBranchTable {
            index,
            targets,
            default_label,
        }) => BlockPyTerm::BranchTable(BlockPyBranchTable {
            index: hoist_core_expr_to_atom(index, out),
            targets,
            default_label,
        }),
        BlockPyTerm::Raise(BlockPyRaise { exc }) => BlockPyTerm::Raise(BlockPyRaise {
            exc: exc.map(|value| hoist_core_expr_to_atom(value, out)),
        }),
        BlockPyTerm::Return(value) => {
            BlockPyTerm::Return(value.map(|value| hoist_core_expr_to_atom(value, out)))
        }
    }
}

fn make_eval_order_explicit_in_core_block<M: Clone + std::fmt::Debug>(
    block: CfgBlock<BlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>, M>,
) -> CfgBlock<BlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>, M> {
    let CfgBlock {
        label,
        body: input_body,
        term: input_term,
        params,
        meta,
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
        meta,
    }
}

pub(crate) fn make_eval_order_explicit_in_core_callable_def(
    callable_def: BlockPyFunction<CoreBlockPyPass>,
) -> BlockPyFunction<CoreBlockPyPass> {
    callable_def.map_blocks(make_eval_order_explicit_in_core_block)
}

fn is_core_atom_without_await(expr: &CoreBlockPyExprWithoutAwait) -> bool {
    matches!(
        expr,
        CoreBlockPyExprWithoutAwait::Name(_) | CoreBlockPyExprWithoutAwait::Literal(_)
    )
}

fn expr_contains_yield(expr: &CoreBlockPyExprWithoutAwait) -> bool {
    match expr {
        CoreBlockPyExprWithoutAwait::Name(_) | CoreBlockPyExprWithoutAwait::Literal(_) => false,
        CoreBlockPyExprWithoutAwait::Call(call) => {
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
        CoreBlockPyExprWithoutAwait::Yield(_) => true,
        CoreBlockPyExprWithoutAwait::YieldFrom(_) => true,
    }
}

fn hoist_core_expr_without_await_to_atom(
    expr: CoreBlockPyExprWithoutAwait,
    out: &mut Vec<BlockPyStmt<CoreBlockPyExprWithoutAwait>>,
) -> CoreBlockPyExprWithoutAwait {
    let expr = make_eval_order_explicit_in_core_expr_without_await(expr, out);
    if is_core_atom_without_await(&expr) {
        expr
    } else {
        let target = fresh_eval_name();
        out.push(BlockPyStmt::Assign(BlockPyAssign {
            target: target.clone(),
            value: expr,
        }));
        CoreBlockPyExprWithoutAwait::Name(target)
    }
}

fn make_eval_order_explicit_in_core_expr_without_await(
    expr: CoreBlockPyExprWithoutAwait,
    out: &mut Vec<BlockPyStmt<CoreBlockPyExprWithoutAwait>>,
) -> CoreBlockPyExprWithoutAwait {
    match expr {
        CoreBlockPyExprWithoutAwait::Name(_) | CoreBlockPyExprWithoutAwait::Literal(_) => expr,
        CoreBlockPyExprWithoutAwait::Call(call) => {
            CoreBlockPyExprWithoutAwait::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(hoist_core_expr_without_await_to_atom(*call.func, out)),
                args: call
                    .args
                    .into_iter()
                    .map(|arg| match arg {
                        CoreBlockPyCallArg::Positional(value) => CoreBlockPyCallArg::Positional(
                            hoist_core_expr_without_await_to_atom(value, out),
                        ),
                        CoreBlockPyCallArg::Starred(value) => CoreBlockPyCallArg::Starred(
                            hoist_core_expr_without_await_to_atom(value, out),
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
                                value: hoist_core_expr_without_await_to_atom(value, out),
                            }
                        }
                        CoreBlockPyKeywordArg::Starred(value) => CoreBlockPyKeywordArg::Starred(
                            hoist_core_expr_without_await_to_atom(value, out),
                        ),
                    })
                    .collect(),
            })
        }
        CoreBlockPyExprWithoutAwait::Yield(yield_expr) => {
            CoreBlockPyExprWithoutAwait::Yield(CoreBlockPyYield {
                node_index: yield_expr.node_index,
                range: yield_expr.range,
                value: yield_expr
                    .value
                    .map(|value| Box::new(hoist_core_expr_without_await_to_atom(*value, out))),
            })
        }
        CoreBlockPyExprWithoutAwait::YieldFrom(yield_from_expr) => {
            CoreBlockPyExprWithoutAwait::YieldFrom(CoreBlockPyYieldFrom {
                node_index: yield_from_expr.node_index,
                range: yield_from_expr.range,
                value: Box::new(hoist_core_expr_without_await_to_atom(
                    *yield_from_expr.value,
                    out,
                )),
            })
        }
    }
}

fn make_eval_order_explicit_in_core_stmt_without_await(
    stmt: BlockPyStmt<CoreBlockPyExprWithoutAwait>,
    out: &mut Vec<BlockPyStmt<CoreBlockPyExprWithoutAwait>>,
) {
    match stmt {
        BlockPyStmt::Assign(assign) => {
            let value = if expr_contains_yield(&assign.value) {
                make_eval_order_explicit_in_core_expr_without_await(assign.value, out)
            } else {
                assign.value
            };
            out.push(BlockPyStmt::Assign(BlockPyAssign {
                target: assign.target,
                value,
            }));
        }
        BlockPyStmt::Expr(expr) => {
            let expr = if expr_contains_yield(&expr) {
                make_eval_order_explicit_in_core_expr_without_await(expr, out)
            } else {
                expr
            };
            out.push(BlockPyStmt::Expr(expr));
        }
        BlockPyStmt::Delete(delete) => out.push(BlockPyStmt::Delete(delete)),
        BlockPyStmt::If(if_stmt) => {
            let test = hoist_core_expr_without_await_to_atom(if_stmt.test, out);
            out.push(BlockPyStmt::If(BlockPyIf {
                test,
                body: make_eval_order_explicit_in_core_fragment_without_await(if_stmt.body),
                orelse: make_eval_order_explicit_in_core_fragment_without_await(if_stmt.orelse),
            }));
        }
    }
}

fn make_eval_order_explicit_in_core_fragment_without_await(
    fragment: BlockPyCfgFragment<
        BlockPyStmt<CoreBlockPyExprWithoutAwait>,
        BlockPyTerm<CoreBlockPyExprWithoutAwait>,
    >,
) -> BlockPyCfgFragment<
    BlockPyStmt<CoreBlockPyExprWithoutAwait>,
    BlockPyTerm<CoreBlockPyExprWithoutAwait>,
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
    term: BlockPyTerm<CoreBlockPyExprWithoutAwait>,
    out: &mut Vec<BlockPyStmt<CoreBlockPyExprWithoutAwait>>,
) -> BlockPyTerm<CoreBlockPyExprWithoutAwait> {
    match term {
        BlockPyTerm::Jump(_) | BlockPyTerm::TryJump(_) => term,
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test,
            then_label,
            else_label,
        }) => BlockPyTerm::IfTerm(BlockPyIfTerm {
            test: hoist_core_expr_without_await_to_atom(test, out),
            then_label,
            else_label,
        }),
        BlockPyTerm::BranchTable(BlockPyBranchTable {
            index,
            targets,
            default_label,
        }) => BlockPyTerm::BranchTable(BlockPyBranchTable {
            index: hoist_core_expr_without_await_to_atom(index, out),
            targets,
            default_label,
        }),
        BlockPyTerm::Raise(BlockPyRaise { exc }) => BlockPyTerm::Raise(BlockPyRaise {
            exc: exc.map(|value| hoist_core_expr_without_await_to_atom(value, out)),
        }),
        BlockPyTerm::Return(value) => BlockPyTerm::Return(
            value.map(|value| hoist_core_expr_without_await_to_atom(value, out)),
        ),
    }
}

pub(crate) fn make_eval_order_explicit_in_core_block_without_await<M: Clone + std::fmt::Debug>(
    block: CfgBlock<
        BlockPyStmt<CoreBlockPyExprWithoutAwait>,
        BlockPyTerm<CoreBlockPyExprWithoutAwait>,
        M,
    >,
) -> CfgBlock<BlockPyStmt<CoreBlockPyExprWithoutAwait>, BlockPyTerm<CoreBlockPyExprWithoutAwait>, M>
{
    let CfgBlock {
        label,
        body: input_body,
        term: input_term,
        params,
        meta,
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
        meta,
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
    use crate::block_py::{BlockPyBlock, BlockPyLabel, BlockPyTerm, CoreBlockPyExpr};

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
            term: BlockPyTerm::Return(Some(CoreBlockPyExpr::from(crate::py_expr!(
                "f(g(x), h(y))"
            )))),
            params: Vec::new(),
            meta: Default::default(),
        };

        let lowered = make_eval_order_explicit_in_core_block(block);
        assert_eq!(lowered.body.len(), 3);
        assert!(matches!(lowered.body[0], BlockPyStmt::Assign(_)));
        assert!(matches!(lowered.body[1], BlockPyStmt::Assign(_)));
        let BlockPyStmt::Assign(assign) = &lowered.body[2] else {
            panic!("expected hoisted call assignment");
        };
        let CoreBlockPyExpr::Call(call) = &assign.value else {
            panic!("expected call expr");
        };
        assert!(matches!(call.func.as_ref(), CoreBlockPyExpr::Name(_)));
        assert!(call.args.iter().all(|arg| matches!(
            arg,
            CoreBlockPyCallArg::Positional(CoreBlockPyExpr::Name(_) | CoreBlockPyExpr::Literal(_))
                | CoreBlockPyCallArg::Starred(
                    CoreBlockPyExpr::Name(_) | CoreBlockPyExpr::Literal(_)
                )
        )));
    }

    #[test]
    fn eval_order_hoists_return_value_to_temp() {
        let block = BlockPyBlock {
            label: BlockPyLabel("start".to_string()),
            body: Vec::new(),
            term: BlockPyTerm::Return(Some(CoreBlockPyExpr::from(crate::py_expr!("f(g(x))")))),
            params: Vec::new(),
            meta: Default::default(),
        };

        let lowered = make_eval_order_explicit_in_core_block(block);
        assert_eq!(lowered.body.len(), 2);
        assert!(matches!(lowered.body[0], BlockPyStmt::Assign(_)));
        assert!(matches!(lowered.body[1], BlockPyStmt::Assign(_)));
        let BlockPyTerm::Return(Some(CoreBlockPyExpr::Name(_))) = lowered.term else {
            panic!("expected return of temp name");
        };
    }

    #[test]
    fn eval_order_leaves_plain_assignment_rhs_untouched() {
        let block = BlockPyBlock {
            label: BlockPyLabel("start".to_string()),
            body: vec![BlockPyStmt::Assign(BlockPyAssign {
                target: fresh_eval_name(),
                value: CoreBlockPyExpr::from(crate::py_expr!("f(g(x))")),
            })],
            term: BlockPyTerm::Return(None),
            params: Vec::new(),
            meta: Default::default(),
        };

        let lowered = make_eval_order_explicit_in_core_block(block);
        assert_eq!(lowered.body.len(), 1);
        let BlockPyStmt::Assign(assign) = &lowered.body[0] else {
            panic!("expected assignment");
        };
        let CoreBlockPyExpr::Call(call) = &assign.value else {
            panic!("expected call");
        };
        let CoreBlockPyCallArg::Positional(CoreBlockPyExpr::Call(inner)) = &call.args[0] else {
            panic!("expected nested call");
        };
        assert!(matches!(call.func.as_ref(), CoreBlockPyExpr::Name(_)));
        assert!(matches!(inner.func.as_ref(), CoreBlockPyExpr::Name(_)));
    }

    #[test]
    fn eval_order_hoists_await_in_assignment_call_argument() {
        let block = BlockPyBlock {
            label: BlockPyLabel("start".to_string()),
            body: vec![BlockPyStmt::Assign(BlockPyAssign {
                target: test_name("total"),
                value: CoreBlockPyExpr::from(crate::py_expr!("__dp_iadd(total, await Once())")),
            })],
            term: BlockPyTerm::Return(None),
            params: Vec::new(),
            meta: Default::default(),
        };

        let lowered = make_eval_order_explicit_in_core_block(block);
        assert_eq!(lowered.body.len(), 3);
        let BlockPyStmt::Assign(first_assign) = &lowered.body[0] else {
            panic!("expected hoisted call temp assignment");
        };
        assert!(matches!(first_assign.value, CoreBlockPyExpr::Call(_)));
        let BlockPyStmt::Assign(temp_assign) = &lowered.body[1] else {
            panic!("expected hoisted await temp assignment");
        };
        assert!(matches!(temp_assign.value, CoreBlockPyExpr::Await(_)));
        let BlockPyStmt::Assign(assign) = &lowered.body[2] else {
            panic!("expected rewritten assignment");
        };
        let CoreBlockPyExpr::Call(call) = &assign.value else {
            panic!("expected iadd call");
        };
        assert!(matches!(
            call.args[1],
            CoreBlockPyCallArg::Positional(CoreBlockPyExpr::Name(_))
        ));
    }

    #[test]
    fn eval_order_without_await_hoists_yield_from_in_assignment_call_argument() {
        let block = BlockPyBlock {
            label: BlockPyLabel("start".to_string()),
            body: vec![BlockPyStmt::Assign(BlockPyAssign {
                target: test_name("total"),
                value: CoreBlockPyExprWithoutAwait::Call(CoreBlockPyCall {
                    node_index: Default::default(),
                    range: Default::default(),
                    func: Box::new(CoreBlockPyExprWithoutAwait::Name(test_name("__dp_iadd"))),
                    args: vec![
                        CoreBlockPyCallArg::Positional(CoreBlockPyExprWithoutAwait::Name(
                            test_name("total"),
                        )),
                        CoreBlockPyCallArg::Positional(CoreBlockPyExprWithoutAwait::YieldFrom(
                            CoreBlockPyYieldFrom {
                                node_index: Default::default(),
                                range: Default::default(),
                                value: Box::new(CoreBlockPyExprWithoutAwait::Name(test_name("it"))),
                            },
                        )),
                    ],
                    keywords: Vec::new(),
                }),
            })],
            term: BlockPyTerm::Return(None),
            params: Vec::new(),
            meta: Default::default(),
        };

        let lowered = make_eval_order_explicit_in_core_block_without_await(block);
        assert_eq!(lowered.body.len(), 2);
        let BlockPyStmt::Assign(temp_assign) = &lowered.body[0] else {
            panic!("expected hoisted yield-from temp assignment");
        };
        assert!(matches!(
            temp_assign.value,
            CoreBlockPyExprWithoutAwait::YieldFrom(_)
        ));
        let BlockPyStmt::Assign(assign) = &lowered.body[1] else {
            panic!("expected rewritten assignment");
        };
        let CoreBlockPyExprWithoutAwait::Call(call) = &assign.value else {
            panic!("expected iadd call");
        };
        assert!(matches!(
            call.args[1],
            CoreBlockPyCallArg::Positional(CoreBlockPyExprWithoutAwait::Name(_))
        ));
    }
}
