use super::{
    BlockParam, BlockParamRole, BlockPyCfgFragment, BlockPyIfTerm, BlockPyLabel, BlockPyNameLike,
    BlockPyStmt, BlockPyTerm, CfgBlock, ImplicitNoneExpr,
};
use crate::block_py::dataflow::{
    assigned_names_in_blockpy_fragment, assigned_names_in_blockpy_stmts,
};
use ruff_python_ast::Expr;
use std::collections::{HashMap, HashSet};

fn blockpy_successors<E, N>(block: &CfgBlock<BlockPyStmt<E, N>, BlockPyTerm<E>>) -> Vec<String> {
    match &block.term {
        BlockPyTerm::Jump(target) => vec![target.as_str().to_string()],
        BlockPyTerm::IfTerm(if_term) => vec![
            if_term.then_label.as_str().to_string(),
            if_term.else_label.as_str().to_string(),
        ],
        BlockPyTerm::BranchTable(branch) => {
            let mut out = branch
                .targets
                .iter()
                .map(|label| label.as_str().to_string())
                .collect::<Vec<_>>();
            out.push(branch.default_label.as_str().to_string());
            out
        }
        BlockPyTerm::Raise(_) | BlockPyTerm::Return(_) => Vec::new(),
    }
}

fn fresh_linearized_if_label(
    base: &BlockPyLabel,
    counter: &mut usize,
    suffix: &str,
) -> BlockPyLabel {
    let label = BlockPyLabel::from(format!("{}_{}_{}", base.as_str(), suffix, *counter));
    *counter += 1;
    label
}

fn extend_ordered_state(base: &[String], assigned: HashSet<String>) -> Vec<String> {
    let mut out = base.to_vec();
    let mut assigned = assigned.into_iter().collect::<Vec<_>>();
    assigned.sort();
    for name in assigned {
        if !out.iter().any(|existing| existing == &name) {
            out.push(name);
        }
    }
    out
}

fn conservative_state_after_prefix<E, N>(base: &[String], body: &[BlockPyStmt<E, N>]) -> Vec<String>
where
    E: Clone + Into<Expr>,
    N: BlockPyNameLike,
{
    extend_ordered_state(base, assigned_names_in_blockpy_stmts(body))
}

fn conservative_state_after_if_branches<E, N>(
    base: &[String],
    if_stmt: &super::BlockPyStructuredIf<E, N>,
) -> Vec<String>
where
    E: Clone + Into<Expr>,
    N: BlockPyNameLike,
{
    let mut assigned = assigned_names_in_blockpy_fragment(&if_stmt.body);
    assigned.extend(assigned_names_in_blockpy_fragment(&if_stmt.orelse));
    extend_ordered_state(base, assigned)
}

fn params_for_linearized_names(
    param_names: &[String],
    declared_params: &[BlockParam],
) -> Vec<BlockParam> {
    param_names
        .iter()
        .map(|name| BlockParam {
            name: name.clone(),
            role: declared_params
                .iter()
                .find(|param| param.name == *name)
                .map(|param| param.role)
                .unwrap_or(BlockParamRole::Local),
        })
        .collect()
}

fn linearize_blockpy_if_sequence<E, N>(
    label: BlockPyLabel,
    body: Vec<BlockPyStmt<E, N>>,
    final_term: BlockPyTerm<E>,
    exc_edge: Option<super::BlockPyEdge>,
    block_params: Vec<String>,
    declared_params: Vec<BlockParam>,
    exc_target: Option<String>,
    next_label_id: &mut usize,
    out_blocks: &mut Vec<CfgBlock<BlockPyStmt<E, N>, BlockPyTerm<E>>>,
    out_block_params: &mut HashMap<String, Vec<String>>,
    out_exception_edges: &mut HashMap<String, Option<String>>,
) where
    E: Clone + Into<Expr>,
    N: BlockPyNameLike,
{
    let Some(if_index) = body
        .iter()
        .position(|stmt| matches!(stmt, BlockPyStmt::If(_)))
    else {
        out_block_params.insert(label.as_str().to_string(), block_params.clone());
        out_exception_edges.insert(label.as_str().to_string(), exc_target);
        out_blocks.push(CfgBlock {
            label,
            body,
            term: final_term,
            params: params_for_linearized_names(&block_params, &declared_params),
            exc_edge,
        });
        return;
    };

    let mut body = body;
    let rest = body.split_off(if_index + 1);
    let if_stmt = match body.pop() {
        Some(BlockPyStmt::If(if_stmt)) => if_stmt,
        _ => unreachable!("expected structured BlockPy if at split point"),
    };
    let available_before_if = conservative_state_after_prefix(&block_params, &body);
    let join_block_params = conservative_state_after_if_branches(&available_before_if, &if_stmt);

    let then_label = fresh_linearized_if_label(&label, next_label_id, "if_then");
    let else_label = fresh_linearized_if_label(&label, next_label_id, "if_else");
    let join_label = if rest.is_empty() {
        None
    } else {
        Some(fresh_linearized_if_label(&label, next_label_id, "if_join"))
    };

    out_block_params.insert(label.as_str().to_string(), block_params);
    out_exception_edges.insert(label.as_str().to_string(), exc_target.clone());
    out_blocks.push(CfgBlock {
        label: label.clone(),
        body,
        term: BlockPyTerm::IfTerm(BlockPyIfTerm {
            test: if_stmt.test.clone(),
            then_label: then_label.clone(),
            else_label: else_label.clone(),
        }),
        params: params_for_linearized_names(
            out_block_params.get(label.as_str()).unwrap(),
            &declared_params,
        ),
        exc_edge: exc_edge.clone(),
    });

    let branch_fallthrough = join_label
        .clone()
        .map(|label| BlockPyTerm::Jump(label.into()))
        .unwrap_or_else(|| final_term.clone());
    linearize_blockpy_fragment(
        then_label,
        if_stmt.body,
        branch_fallthrough.clone(),
        exc_edge.clone(),
        available_before_if.clone(),
        declared_params.clone(),
        exc_target.clone(),
        next_label_id,
        out_blocks,
        out_block_params,
        out_exception_edges,
    );
    linearize_blockpy_fragment(
        else_label,
        if_stmt.orelse,
        branch_fallthrough,
        exc_edge.clone(),
        available_before_if.clone(),
        declared_params.clone(),
        exc_target.clone(),
        next_label_id,
        out_blocks,
        out_block_params,
        out_exception_edges,
    );

    if let Some(join_label) = join_label {
        linearize_blockpy_if_sequence(
            join_label,
            rest,
            final_term,
            exc_edge,
            join_block_params,
            declared_params,
            exc_target,
            next_label_id,
            out_blocks,
            out_block_params,
            out_exception_edges,
        );
    }
}

fn linearize_blockpy_fragment<E, N>(
    label: BlockPyLabel,
    fragment: BlockPyCfgFragment<BlockPyStmt<E, N>, BlockPyTerm<E>>,
    fallthrough_term: BlockPyTerm<E>,
    exc_edge: Option<super::BlockPyEdge>,
    block_params: Vec<String>,
    declared_params: Vec<BlockParam>,
    exc_target: Option<String>,
    next_label_id: &mut usize,
    out_blocks: &mut Vec<CfgBlock<BlockPyStmt<E, N>, BlockPyTerm<E>>>,
    out_block_params: &mut HashMap<String, Vec<String>>,
    out_exception_edges: &mut HashMap<String, Option<String>>,
) where
    E: Clone + Into<Expr>,
    N: BlockPyNameLike,
{
    linearize_blockpy_if_sequence(
        label,
        fragment.body,
        fragment.term.unwrap_or(fallthrough_term),
        exc_edge,
        block_params,
        declared_params,
        exc_target,
        next_label_id,
        out_blocks,
        out_block_params,
        out_exception_edges,
    );
}

pub(crate) fn linearize_structured_ifs<E, N>(
    blocks: &[CfgBlock<BlockPyStmt<E, N>, BlockPyTerm<E>>],
    block_params: &HashMap<String, Vec<String>>,
    exception_edges: &HashMap<String, Option<String>>,
) -> (
    Vec<CfgBlock<BlockPyStmt<E, N>, BlockPyTerm<E>>>,
    HashMap<String, Vec<String>>,
    HashMap<String, Option<String>>,
)
where
    E: Clone + Into<Expr>,
    N: BlockPyNameLike,
{
    let mut out_blocks = Vec::new();
    let mut out_block_params = HashMap::new();
    let mut out_exception_edges = HashMap::new();
    let mut next_label_id = 0usize;
    for block in blocks {
        let mut params = block_params
            .get(block.label.as_str())
            .cloned()
            .unwrap_or_default();
        for name in block.bb_param_names() {
            if !params.iter().any(|existing| existing == name) {
                params.push(name.to_string());
            }
        }
        let exc_target = exception_edges
            .get(block.label.as_str())
            .cloned()
            .unwrap_or(None);
        linearize_blockpy_if_sequence(
            block.label.clone(),
            block.body.clone(),
            block.term.clone(),
            block.exc_edge.clone(),
            params,
            block.params.clone(),
            exc_target,
            &mut next_label_id,
            &mut out_blocks,
            &mut out_block_params,
            &mut out_exception_edges,
        );
    }
    (out_blocks, out_block_params, out_exception_edges)
}

pub(crate) fn fold_jumps_to_trivial_none_return_blockpy<E, N>(
    blocks: &mut [CfgBlock<BlockPyStmt<E, N>, BlockPyTerm<E>>],
) where
    E: Clone + ImplicitNoneExpr,
    N: BlockPyNameLike,
{
    let trivial_ret_none_terms: HashMap<String, BlockPyTerm<E>> = blocks
        .iter()
        .filter(|block| {
            block.body.is_empty()
                && match &block.term {
                    BlockPyTerm::Return(expr) => E::is_implicit_none_expr(expr),
                    _ => false,
                }
        })
        .map(|block| (block.label.as_str().to_string(), block.term.clone()))
        .collect();

    for block in blocks.iter_mut() {
        let jump_target = match &block.term {
            BlockPyTerm::Jump(target) => Some(target.as_str().to_string()),
            _ => None,
        };
        if let Some(target) = jump_target {
            if let Some(term) = trivial_ret_none_terms.get(target.as_str()) {
                block.term = term.clone();
            }
        }
    }
}

pub(crate) fn fold_constant_brif_blockpy(
    blocks: &mut [CfgBlock<BlockPyStmt<Expr>, BlockPyTerm<Expr>>],
) {
    for block in blocks.iter_mut() {
        let jump_target = match &block.term {
            BlockPyTerm::IfTerm(BlockPyIfTerm {
                test,
                then_label,
                else_label,
            }) => match test {
                Expr::BooleanLiteral(boolean) => {
                    if boolean.value {
                        Some(then_label.as_str().to_string())
                    } else {
                        Some(else_label.as_str().to_string())
                    }
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(target) = jump_target {
            block.term = BlockPyTerm::Jump(BlockPyLabel::from(target).into());
        }
    }
}

pub(crate) fn prune_unreachable_blockpy_blocks<E>(
    entry_label: &str,
    extra_roots: &[String],
    blocks: &mut Vec<CfgBlock<BlockPyStmt<E>, BlockPyTerm<E>>>,
) {
    let index_by_label: HashMap<String, usize> = blocks
        .iter()
        .enumerate()
        .map(|(idx, block)| (block.label.as_str().to_string(), idx))
        .collect();

    let mut worklist = vec![entry_label.to_string()];
    worklist.extend(extra_roots.iter().cloned());
    let mut reachable = HashSet::new();
    while let Some(label) = worklist.pop() {
        if !reachable.insert(label.clone()) {
            continue;
        }
        let Some(idx) = index_by_label.get(label.as_str()) else {
            continue;
        };
        for succ in blockpy_successors(&blocks[*idx]) {
            worklist.push(succ);
        }
    }

    blocks.retain(|block| reachable.contains(block.label.as_str()));
}
