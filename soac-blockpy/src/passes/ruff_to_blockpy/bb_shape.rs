use crate::block_py::cfg::linearize_structured_ifs;
use crate::block_py::operation;
use crate::block_py::{
    BlockArg, BlockPyEdge, BlockPyIfTerm, BlockPyNameLike, BlockPyStmt, BlockPyTerm,
    CoreBlockPyCallArg, CoreBlockPyExpr, CoreBlockPyExprWithAwaitAndYield, CoreBlockPyLiteral,
    StructuredBlockPyStmt,
};
use ruff_python_ast::{self as ast};
use ruff_text_size::TextRange;
use std::collections::{HashMap, HashSet};

pub(crate) fn lower_structured_blocks_to_bb_blocks<E, N>(
    blocks: &[crate::block_py::CfgBlock<StructuredBlockPyStmt<E, N>, BlockPyTerm<E>>],
) -> Vec<crate::block_py::CfgBlock<BlockPyStmt<E, N>, BlockPyTerm<E>>>
where
    E: Clone,
    N: BlockPyNameLike,
{
    let exception_edges = lowered_exception_edges(blocks);
    let (linear_blocks, _linear_block_params, linear_exception_edges) =
        linearize_structured_ifs(blocks, &HashMap::new(), &exception_edges);
    let mut bb_blocks = linear_blocks
        .iter()
        .map(|block| {
            let exc_edge = linear_exception_edges
                .get(&block.label)
                .cloned()
                .flatten()
                .map(BlockPyEdge::new);
            let ops = block
                .body
                .clone()
                .into_iter()
                .map(BlockPyStmt::from)
                .collect::<Vec<_>>();
            crate::block_py::CfgBlock {
                label: block.label.clone(),
                body: ops,
                term: block.term.clone(),
                params: block.bb_params().cloned().collect(),
                exc_edge,
            }
        })
        .collect::<Vec<_>>();
    populate_exception_edge_args(&mut bb_blocks);
    bb_blocks
}

pub(crate) fn rewrite_current_exception_in_core_blocks<N>(
    blocks: &mut [crate::block_py::CfgBlock<
        BlockPyStmt<CoreBlockPyExpr<N>, N>,
        BlockPyTerm<CoreBlockPyExpr<N>>,
    >],
) where
    N: BlockPyNameLike,
{
    for block in blocks {
        let Some(exc_name) = block.exception_param().map(ToString::to_string) else {
            continue;
        };
        for stmt in &mut block.body {
            rewrite_current_exception_in_bb_stmt(stmt, exc_name.as_str());
        }
        rewrite_current_exception_in_blockpy_term(&mut block.term, exc_name.as_str());
    }
}

pub(crate) fn rewrite_current_exception_in_core_blocks_with_await_and_yield(
    blocks: &mut [crate::block_py::CfgBlock<
        BlockPyStmt<CoreBlockPyExprWithAwaitAndYield, ast::ExprName>,
        BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
    >],
) {
    for block in blocks {
        let Some(exc_name) = block.exception_param().map(ToString::to_string) else {
            continue;
        };
        for stmt in &mut block.body {
            match stmt {
                BlockPyStmt::Assign(assign) => {
                    rewrite_current_exception_in_expr_with_await_and_yield(
                        &mut assign.value,
                        exc_name.as_str(),
                    );
                }
                BlockPyStmt::Expr(expr) => {
                    rewrite_current_exception_in_expr_with_await_and_yield(expr, exc_name.as_str())
                }
                BlockPyStmt::Delete(_) => {}
            }
        }
        rewrite_current_exception_in_term_with_await_and_yield(&mut block.term, exc_name.as_str());
    }
}

fn rewrite_current_exception_in_bb_stmt<N>(
    stmt: &mut BlockPyStmt<CoreBlockPyExpr<N>, N>,
    exc_name: &str,
) where
    N: BlockPyNameLike,
{
    match stmt {
        BlockPyStmt::Assign(assign) => {
            rewrite_current_exception_in_blockpy_expr(&mut assign.value, exc_name);
        }
        BlockPyStmt::Expr(expr) => {
            rewrite_current_exception_in_blockpy_expr(expr, exc_name);
        }
        BlockPyStmt::Delete(_) => {}
    }
}

fn rewrite_current_exception_in_term_with_await_and_yield(
    term: &mut BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
    exc_name: &str,
) {
    match term {
        BlockPyTerm::IfTerm(BlockPyIfTerm { test, .. }) => {
            rewrite_current_exception_in_expr_with_await_and_yield(test, exc_name);
        }
        BlockPyTerm::BranchTable(branch) => {
            rewrite_current_exception_in_expr_with_await_and_yield(&mut branch.index, exc_name);
        }
        BlockPyTerm::Raise(raise_stmt) => {
            if let Some(exc) = raise_stmt.exc.as_mut() {
                rewrite_current_exception_in_expr_with_await_and_yield(exc, exc_name);
            } else {
                raise_stmt.exc = Some(CoreBlockPyExprWithAwaitAndYield::Name(ast::ExprName {
                    id: exc_name.into(),
                    ctx: ast::ExprContext::Load,
                    range: Default::default(),
                    node_index: ast::AtomicNodeIndex::default(),
                }));
            }
        }
        BlockPyTerm::Return(value) => {
            rewrite_current_exception_in_expr_with_await_and_yield(value, exc_name);
        }
        BlockPyTerm::Jump(_) => {}
    }
}

fn rewrite_current_exception_in_expr_with_await_and_yield(
    expr: &mut CoreBlockPyExprWithAwaitAndYield,
    exc_name: &str,
) {
    match expr {
        CoreBlockPyExprWithAwaitAndYield::Op(operation) => {
            operation.walk_args_mut(&mut |arg| {
                rewrite_current_exception_in_expr_with_await_and_yield(arg, exc_name)
            });
        }
        CoreBlockPyExprWithAwaitAndYield::Await(await_expr) => {
            rewrite_current_exception_in_expr_with_await_and_yield(
                await_expr.value.as_mut(),
                exc_name,
            );
        }
        CoreBlockPyExprWithAwaitAndYield::Yield(yield_expr) => {
            if let Some(value) = yield_expr.value.as_mut() {
                rewrite_current_exception_in_expr_with_await_and_yield(value.as_mut(), exc_name);
            }
        }
        CoreBlockPyExprWithAwaitAndYield::YieldFrom(yield_from) => {
            rewrite_current_exception_in_expr_with_await_and_yield(
                yield_from.value.as_mut(),
                exc_name,
            );
        }
        CoreBlockPyExprWithAwaitAndYield::Name(_)
        | CoreBlockPyExprWithAwaitAndYield::Literal(_) => {}
    }

    if is_current_exception_call_with_await_and_yield(expr) {
        *expr = current_exception_name_expr_with_await_and_yield(exc_name);
    } else if is_exc_info_call_with_await_and_yield(expr) {
        *expr = current_exception_info_expr_with_await_and_yield(exc_name);
    }
}

fn is_current_exception_call_with_await_and_yield(expr: &CoreBlockPyExprWithAwaitAndYield) -> bool {
    let CoreBlockPyExprWithAwaitAndYield::Op(operation) = expr else {
        return false;
    };
    let operation::OperationDetail::Call(call) = operation else {
        return false;
    };
    call.args.is_empty()
        && call.keywords.is_empty()
        && expr_root_name_id_with_await_and_yield(call.func.as_ref())
            == Some("__dp_current_exception")
}

fn is_exc_info_call_with_await_and_yield(expr: &CoreBlockPyExprWithAwaitAndYield) -> bool {
    let CoreBlockPyExprWithAwaitAndYield::Op(operation) = expr else {
        return false;
    };
    let operation::OperationDetail::Call(call) = operation else {
        return false;
    };
    call.args.is_empty()
        && call.keywords.is_empty()
        && expr_root_name_id_with_await_and_yield(call.func.as_ref()) == Some("__dp_exc_info")
}

fn current_exception_name_expr_with_await_and_yield(
    exc_name: &str,
) -> CoreBlockPyExprWithAwaitAndYield {
    CoreBlockPyExprWithAwaitAndYield::Name(ast::ExprName {
        id: exc_name.into(),
        ctx: ast::ExprContext::Load,
        range: compat_range(),
        node_index: compat_node_index(),
    })
}

fn current_exception_info_expr_with_await_and_yield(
    exc_name: &str,
) -> CoreBlockPyExprWithAwaitAndYield {
    crate::block_py::core_positional_call_expr_with_meta(
        "__dp_exc_info_from_exception",
        compat_node_index(),
        compat_range(),
        vec![current_exception_name_expr_with_await_and_yield(exc_name)],
    )
}

pub(crate) fn populate_exception_edge_args<E, N>(
    blocks: &mut [crate::block_py::CfgBlock<BlockPyStmt<E, N>, BlockPyTerm<E>>],
) {
    let label_to_index = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.label.clone(), index))
        .collect::<HashMap<_, _>>();
    for block_index in 0..blocks.len() {
        let Some(exc_target_label) = blocks[block_index]
            .exc_edge
            .as_ref()
            .map(|edge| edge.target.clone())
        else {
            continue;
        };
        let Some(target_index) = label_to_index.get(&exc_target_label).copied() else {
            continue;
        };
        let source_params = blocks[block_index].param_name_vec();
        let source_has_owner = source_params
            .iter()
            .any(|param| param == "_dp_self" || param == "_dp_state");
        let target_params = blocks[target_index].param_name_vec();
        let exc_name = blocks[target_index]
            .exception_param()
            .map(ToString::to_string);
        let current_exception_aliases = match &blocks[target_index].term {
            BlockPyTerm::Jump(edge) => edge
                .args
                .iter()
                .filter_map(|arg| match arg {
                    BlockArg::Name(name) if name.starts_with("_dp_try_exc_") => Some(name.as_str()),
                    _ => None,
                })
                .collect::<HashSet<_>>(),
            _ => HashSet::new(),
        };
        let args = target_params
            .into_iter()
            .map(|target_param| {
                if exc_name.as_deref() == Some(target_param.as_str()) {
                    BlockArg::CurrentException
                } else if current_exception_aliases.contains(target_param.as_str()) {
                    BlockArg::CurrentException
                } else if source_params.iter().any(|param| param == &target_param)
                    || source_has_owner
                {
                    BlockArg::Name(target_param)
                } else {
                    BlockArg::None
                }
            })
            .collect();
        blocks[block_index].exc_edge = Some(BlockPyEdge::with_args(exc_target_label, args));
    }
}

pub(crate) fn lowered_exception_edges<S, T>(
    blocks: &[crate::block_py::CfgBlock<S, T>],
) -> HashMap<crate::block_py::BlockPyLabel, Option<crate::block_py::BlockPyLabel>> {
    blocks
        .iter()
        .map(|block| {
            (
                block.label.clone(),
                block.exc_edge.as_ref().map(|edge| edge.target.clone()),
            )
        })
        .collect()
}

fn rewrite_current_exception_in_blockpy_term<N>(
    term: &mut BlockPyTerm<CoreBlockPyExpr<N>>,
    exc_name: &str,
) where
    N: BlockPyNameLike,
{
    match term {
        BlockPyTerm::IfTerm(BlockPyIfTerm { test, .. }) => {
            rewrite_current_exception_in_blockpy_expr(test, exc_name);
        }
        BlockPyTerm::BranchTable(branch) => {
            rewrite_current_exception_in_blockpy_expr(&mut branch.index, exc_name);
        }
        BlockPyTerm::Raise(raise_stmt) => {
            if let Some(exc) = raise_stmt.exc.as_mut() {
                rewrite_current_exception_in_blockpy_expr(exc, exc_name);
            } else {
                raise_stmt.exc = Some(current_exception_name_expr(exc_name));
            }
        }
        BlockPyTerm::Return(value) => rewrite_current_exception_in_blockpy_expr(value, exc_name),
        BlockPyTerm::Jump(_) => {}
    }
}

fn rewrite_current_exception_in_blockpy_expr<N>(expr: &mut CoreBlockPyExpr<N>, exc_name: &str)
where
    N: BlockPyNameLike,
{
    match expr {
        CoreBlockPyExpr::Op(operation) => {
            operation
                .walk_args_mut(&mut |arg| rewrite_current_exception_in_blockpy_expr(arg, exc_name));
        }
        CoreBlockPyExpr::Name(_) | CoreBlockPyExpr::Literal(_) => {}
    }

    if is_current_exception_call(expr) {
        *expr = current_exception_name_expr(exc_name);
    } else if is_exc_info_call(expr) {
        *expr = current_exception_info_expr(exc_name);
    }
}

fn operation_expr<N: BlockPyNameLike + Clone>(
    expr: &CoreBlockPyExpr<N>,
) -> Option<&operation::OperationDetail<CoreBlockPyExpr<N>>> {
    match expr {
        CoreBlockPyExpr::Op(operation) => Some(operation),
        _ => None,
    }
}

fn is_current_exception_call<N>(expr: &CoreBlockPyExpr<N>) -> bool
where
    N: BlockPyNameLike,
{
    let Some(operation) = operation_expr(expr) else {
        return false;
    };
    let operation::OperationDetail::Call(call) = operation else {
        return false;
    };
    call.args.is_empty()
        && call.keywords.is_empty()
        && is_dp_lookup_call_expr(call.func.as_ref(), "current_exception")
}

fn is_exc_info_call<N>(expr: &CoreBlockPyExpr<N>) -> bool
where
    N: BlockPyNameLike,
{
    let Some(operation) = operation_expr(expr) else {
        return false;
    };
    let operation::OperationDetail::Call(call) = operation else {
        return false;
    };
    call.args.is_empty()
        && call.keywords.is_empty()
        && is_dp_lookup_call_expr(call.func.as_ref(), "exc_info")
}

fn is_dp_lookup_call_expr<N>(func: &CoreBlockPyExpr<N>, attr_name: &str) -> bool
where
    N: BlockPyNameLike + Clone,
{
    let helper_name = format!("__dp_{attr_name}");
    expr_root_name_id(func) == Some(helper_name.as_str())
}

fn expr_root_name_id<N>(expr: &CoreBlockPyExpr<N>) -> Option<&str>
where
    N: BlockPyNameLike,
{
    match expr {
        CoreBlockPyExpr::Name(name) => Some(name.id_str()),
        CoreBlockPyExpr::Op(operation) => match operation {
            operation::OperationDetail::LoadRuntime(op) => Some(op.name.as_str()),
            _ => None,
        },
        _ => None,
    }
}

fn expr_root_name_id_with_await_and_yield(expr: &CoreBlockPyExprWithAwaitAndYield) -> Option<&str> {
    match expr {
        CoreBlockPyExprWithAwaitAndYield::Name(name) => Some(name.id.as_str()),
        CoreBlockPyExprWithAwaitAndYield::Op(operation) => match operation {
            operation::OperationDetail::LoadRuntime(op) => Some(op.name.as_str()),
            _ => None,
        },
        _ => None,
    }
}

fn current_exception_name_expr<N>(exc_name: &str) -> CoreBlockPyExpr<N>
where
    N: BlockPyNameLike,
{
    CoreBlockPyExpr::Name(N::from(ast::ExprName {
        id: exc_name.into(),
        ctx: ast::ExprContext::Load,
        range: compat_range(),
        node_index: compat_node_index(),
    }))
}

fn runtime_name_expr<N>(name: &str) -> CoreBlockPyExpr<N>
where
    N: BlockPyNameLike,
{
    CoreBlockPyExpr::Op(operation::LoadRuntime::new(name.to_string()).into())
}

fn current_exception_info_expr<N>(exc_name: &str) -> CoreBlockPyExpr<N>
where
    N: BlockPyNameLike,
{
    crate::block_py::core_positional_call_expr_with_meta(
        "__dp_exc_info_from_exception",
        compat_node_index(),
        compat_range(),
        vec![current_exception_name_expr(exc_name)],
    )
}

fn runtime_name_expr_with_await_and_yield(name: &str) -> CoreBlockPyExprWithAwaitAndYield {
    CoreBlockPyExprWithAwaitAndYield::Op(operation::LoadRuntime::new(name.to_string()).into())
}

fn compat_node_index() -> ast::AtomicNodeIndex {
    ast::AtomicNodeIndex::default()
}

fn compat_range() -> TextRange {
    TextRange::default()
}

#[cfg(test)]
pub(crate) use tests::lower_structured_located_blocks_to_bb_blocks;
#[cfg(test)]
mod tests;
