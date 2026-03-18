use super::block_py::{
    BlockPyBlock, BlockPyBranchTable, BlockPyCallableDef, BlockPyCfgFragment, BlockPyIf,
    BlockPyIfTerm, BlockPyRaise, BlockPyStmt, BlockPyTerm, CoreBlockPyAwait, CoreBlockPyCall,
    CoreBlockPyCallArg, CoreBlockPyExpr, CoreBlockPyKeywordArg, CoreBlockPyYield,
    CoreBlockPyYieldFrom,
};
use super::blockpy_to_bb::LoweredCoreBlockPyFunction;
use super::cfg_ir::CfgCallableDef;
use crate::basic_block::block_py::BlockPyAssign;
use crate::basic_block::cfg_ir::CfgModule;
use crate::namegen::fresh_name;
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
        CoreBlockPyExpr::Await(await_expr) => expr_contains_await_or_yield(&await_expr.value),
        CoreBlockPyExpr::Yield(yield_expr) => yield_expr
            .value
            .as_ref()
            .is_some_and(|value| expr_contains_await_or_yield(value)),
        CoreBlockPyExpr::YieldFrom(yield_from_expr) => {
            expr_contains_await_or_yield(&yield_from_expr.value)
        }
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

fn make_eval_order_explicit_in_core_block(
    block: &BlockPyBlock<CoreBlockPyExpr>,
) -> BlockPyBlock<CoreBlockPyExpr> {
    let mut body = Vec::new();
    for stmt in block.body.clone() {
        make_eval_order_explicit_in_core_stmt(stmt, &mut body);
    }
    let term = make_eval_order_explicit_in_core_term(block.term.clone(), &mut body);
    BlockPyBlock {
        label: block.label.clone(),
        body,
        term,
        meta: block.meta.clone(),
    }
}

fn make_eval_order_explicit_in_core_callable_def(
    callable_def: &BlockPyCallableDef<CoreBlockPyExpr>,
) -> BlockPyCallableDef<CoreBlockPyExpr> {
    BlockPyCallableDef {
        cfg: CfgCallableDef {
            function_id: callable_def.function_id,
            bind_name: callable_def.bind_name.clone(),
            display_name: callable_def.display_name.clone(),
            qualname: callable_def.qualname.clone(),
            kind: callable_def.kind,
            params: callable_def.params.clone(),
            param_defaults: callable_def.param_defaults.clone(),
            entry_liveins: callable_def.entry_liveins.clone(),
            blocks: callable_def
                .blocks
                .iter()
                .map(make_eval_order_explicit_in_core_block)
                .collect(),
        },
        fn_name: callable_def.fn_name.clone(),
        doc: callable_def.doc.clone(),
        capture_names: callable_def.capture_names.clone(),
        closure_layout: callable_def.closure_layout.clone(),
        facts: callable_def.facts.clone(),
        local_cell_slots: callable_def.local_cell_slots.clone(),
        try_regions: callable_def.try_regions.clone(),
    }
}

fn make_eval_order_explicit_in_lowered_core_blockpy_function(
    lowered: &LoweredCoreBlockPyFunction,
) -> LoweredCoreBlockPyFunction {
    lowered.map_callable_def(make_eval_order_explicit_in_core_callable_def)
}

pub(crate) fn make_eval_order_explicit_in_lowered_core_blockpy_module_bundle(
    module: CfgModule<LoweredCoreBlockPyFunction>,
) -> CfgModule<LoweredCoreBlockPyFunction> {
    module.map_callable_defs(make_eval_order_explicit_in_lowered_core_blockpy_function)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basic_block::block_py::{BlockPyLabel, BlockPyTerm, CoreBlockPyExpr};

    #[test]
    fn eval_order_hoists_call_arguments_in_return_value_to_temps() {
        let block = BlockPyBlock {
            label: BlockPyLabel("start".to_string()),
            body: Vec::new(),
            term: BlockPyTerm::Return(Some(CoreBlockPyExpr::from(crate::py_expr!(
                "f(g(x), h(y))"
            )))),
            meta: Default::default(),
        };

        let lowered = make_eval_order_explicit_in_core_block(&block);
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
            meta: Default::default(),
        };

        let lowered = make_eval_order_explicit_in_core_block(&block);
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
            meta: Default::default(),
        };

        let lowered = make_eval_order_explicit_in_core_block(&block);
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
}
