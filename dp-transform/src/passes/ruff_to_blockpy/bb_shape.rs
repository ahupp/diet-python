use crate::block_py::cfg::linearize_structured_ifs;
use crate::block_py::{
    BbStmt, BlockArg, BlockParam, BlockParamRole, BlockPyEdge, BlockPyIfTerm, BlockPyNameLike,
    BlockPyStmt, BlockPyTerm, CoreBlockPyCallArg, CoreBlockPyExpr, CoreBlockPyLiteral,
    IntrinsicCall,
};
use ruff_python_ast::{self as ast};
use ruff_text_size::TextRange;
use std::collections::{HashMap, HashSet};

pub(crate) fn lower_structured_blocks_to_bb_blocks<E, N>(
    blocks: &[crate::block_py::CfgBlock<BlockPyStmt<E, N>, BlockPyTerm<E>>],
    block_params: &HashMap<String, Vec<String>>,
) -> Vec<crate::block_py::CfgBlock<BbStmt<E, N>, BlockPyTerm<E>>>
where
    E: Clone + Into<crate::block_py::Expr>,
    N: BlockPyNameLike,
{
    let exception_edges = lowered_exception_edges(blocks);
    let (linear_blocks, linear_block_params, linear_exception_edges) =
        linearize_structured_ifs(blocks, block_params, &exception_edges);
    let mut bb_blocks = linear_blocks
        .iter()
        .map(|block| {
            let exc_edge = linear_exception_edges
                .get(block.label.as_str())
                .cloned()
                .flatten()
                .map(crate::block_py::BlockPyLabel::from)
                .map(BlockPyEdge::new);
            let ops = block
                .body
                .clone()
                .into_iter()
                .map(BbStmt::from)
                .collect::<Vec<_>>();
            let semantic_param_names = block
                .param_names()
                .map(ToString::to_string)
                .collect::<HashSet<_>>();
            let mut params = linear_block_params
                .get(block.label.as_str())
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|param| !semantic_param_names.contains(param))
                .map(|name| BlockParam {
                    name,
                    role: BlockParamRole::Local,
                })
                .collect::<Vec<_>>();
            params.extend(block.bb_params().cloned());
            crate::block_py::CfgBlock {
                label: block.label.clone(),
                body: ops,
                term: block.term.clone(),
                params,
                exc_edge,
            }
        })
        .collect::<Vec<_>>();
    populate_exception_edge_args(&mut bb_blocks);
    bb_blocks
}

pub(crate) fn rewrite_current_exception_in_core_blocks<N>(
    blocks: &mut [crate::block_py::CfgBlock<
        BbStmt<CoreBlockPyExpr<N>, N>,
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

fn rewrite_current_exception_in_bb_stmt<N>(stmt: &mut BbStmt<CoreBlockPyExpr<N>, N>, exc_name: &str)
where
    N: BlockPyNameLike,
{
    match stmt {
        BbStmt::Assign(assign) => {
            rewrite_current_exception_in_blockpy_expr(&mut assign.value, exc_name);
        }
        BbStmt::Expr(expr) => {
            rewrite_current_exception_in_blockpy_expr(expr, exc_name);
        }
        BbStmt::Delete(_) => {}
    }
}

pub(crate) fn populate_exception_edge_args<E, N>(
    blocks: &mut [crate::block_py::CfgBlock<BbStmt<E, N>, BlockPyTerm<E>>],
) {
    let label_to_index = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.label.as_str().to_string(), index))
        .collect::<HashMap<_, _>>();
    for block_index in 0..blocks.len() {
        let Some(exc_target_label) = blocks[block_index]
            .exc_edge
            .as_ref()
            .map(|edge| edge.target.clone())
        else {
            continue;
        };
        let Some(target_index) = label_to_index.get(exc_target_label.as_str()).copied() else {
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
) -> HashMap<String, Option<String>> {
    blocks
        .iter()
        .map(|block| {
            (
                block.label.as_str().to_string(),
                block.exc_edge.as_ref().map(|edge| edge.target.to_string()),
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
        CoreBlockPyExpr::Call(call) => {
            rewrite_current_exception_in_blockpy_expr(call.func.as_mut(), exc_name);
            for arg in &mut call.args {
                rewrite_current_exception_in_blockpy_expr(arg.expr_mut(), exc_name);
            }
            for keyword in &mut call.keywords {
                rewrite_current_exception_in_blockpy_expr(keyword.expr_mut(), exc_name);
            }
        }
        CoreBlockPyExpr::Intrinsic(IntrinsicCall { args, .. }) => {
            for arg in args {
                rewrite_current_exception_in_blockpy_expr(arg, exc_name);
            }
        }
        CoreBlockPyExpr::Name(_) | CoreBlockPyExpr::Literal(_) => {}
    }

    if is_current_exception_call(expr) {
        *expr = current_exception_name_expr(exc_name);
    } else if is_exc_info_call(expr) {
        *expr = current_exception_info_expr(exc_name);
    }
}

fn is_current_exception_call<N>(expr: &CoreBlockPyExpr<N>) -> bool
where
    N: BlockPyNameLike,
{
    let CoreBlockPyExpr::Call(call) = expr else {
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
    let CoreBlockPyExpr::Call(call) = expr else {
        return false;
    };
    call.args.is_empty()
        && call.keywords.is_empty()
        && is_dp_lookup_call_expr(call.func.as_ref(), "exc_info")
}

fn is_dp_lookup_call_expr<N>(func: &CoreBlockPyExpr<N>, attr_name: &str) -> bool
where
    N: BlockPyNameLike,
{
    match func {
        CoreBlockPyExpr::Name(name) => name.id_str() == format!("__dp_{attr_name}"),
        CoreBlockPyExpr::Call(call) if call.keywords.is_empty() && call.args.len() == 2 => {
            matches!(
                call.func.as_ref(),
                CoreBlockPyExpr::Name(name) if name.id_str() == "__dp_getattr"
            ) && is_dp_getattr_lookup_args(&call.args, attr_name)
        }
        CoreBlockPyExpr::Intrinsic(IntrinsicCall {
            intrinsic, args, ..
        }) if args.len() == 2 && intrinsic.name() == "__dp_getattr" => {
            is_dp_getattr_intrinsic_args(args, attr_name)
        }
        _ => false,
    }
}

fn is_dp_getattr_lookup_args<N>(
    args: &[CoreBlockPyCallArg<CoreBlockPyExpr<N>>],
    attr_name: &str,
) -> bool
where
    N: BlockPyNameLike,
{
    matches!(
        &args[0],
        CoreBlockPyCallArg::Positional(CoreBlockPyExpr::Name(base))
            if base.id_str() == "__dp__"
    ) && expr_static_str(match &args[1] {
        CoreBlockPyCallArg::Positional(value) => value,
        CoreBlockPyCallArg::Starred(_) => return false,
    }) == Some(attr_name.to_string())
}

fn is_dp_getattr_intrinsic_args<N>(args: &[CoreBlockPyExpr<N>], attr_name: &str) -> bool
where
    N: BlockPyNameLike,
{
    matches!(
        &args[0],
        CoreBlockPyExpr::Name(base) if base.id_str() == "__dp__"
    ) && expr_static_str(&args[1]) == Some(attr_name.to_string())
}

fn expr_static_str<N>(expr: &CoreBlockPyExpr<N>) -> Option<String>
where
    N: BlockPyNameLike,
{
    match expr {
        CoreBlockPyExpr::Literal(CoreBlockPyLiteral::StringLiteral(value)) => {
            Some(value.value.clone())
        }
        CoreBlockPyExpr::Literal(CoreBlockPyLiteral::BytesLiteral(bytes)) => {
            String::from_utf8(bytes.value.clone()).ok()
        }
        CoreBlockPyExpr::Call(call)
            if call.keywords.is_empty()
                && call.args.len() == 1
                && matches!(
                    call.func.as_ref(),
                    CoreBlockPyExpr::Name(name)
                        if matches!(
                            name.id_str(),
                            "__dp_decode_literal_bytes" | "__dp_decode_literal_source_bytes"
                        )
                ) =>
        {
            match &call.args[0] {
                CoreBlockPyCallArg::Positional(CoreBlockPyExpr::Literal(
                    CoreBlockPyLiteral::BytesLiteral(bytes),
                )) => String::from_utf8(bytes.value.clone()).ok(),
                _ => None,
            }
        }
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

fn current_exception_info_expr<N>(exc_name: &str) -> CoreBlockPyExpr<N>
where
    N: BlockPyNameLike,
{
    CoreBlockPyExpr::Call(crate::block_py::CoreBlockPyCall {
        node_index: compat_node_index(),
        range: compat_range(),
        func: Box::new(CoreBlockPyExpr::Name(N::from(ast::ExprName {
            id: "__dp_exc_info_from_exception".into(),
            ctx: ast::ExprContext::Load,
            range: compat_range(),
            node_index: compat_node_index(),
        }))),
        args: vec![CoreBlockPyCallArg::Positional(current_exception_name_expr(
            exc_name,
        ))],
        keywords: Vec::new(),
    })
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
