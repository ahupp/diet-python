use super::{
    BlockParam, BlockParamRole, BlockPyCfgFragment, BlockPyIfTerm, BlockPyLabel, BlockPyNameLike,
    BlockPyTerm, CfgBlock, ImplicitNoneExpr, StructuredBlockPyStmt,
};
use crate::block_py::dataflow::{
    assigned_names_in_blockpy_fragment, assigned_names_in_blockpy_stmts,
};
use ruff_python_ast::Expr;
use std::collections::{HashMap, HashSet};

fn blockpy_successors<E, N>(
    block: &CfgBlock<StructuredBlockPyStmt<E, N>, BlockPyTerm<E>>,
) -> Vec<BlockPyLabel> {
    match &block.term {
        BlockPyTerm::Jump(target) => vec![target.target.clone()],
        BlockPyTerm::IfTerm(if_term) => {
            vec![if_term.then_label.clone(), if_term.else_label.clone()]
        }
        BlockPyTerm::BranchTable(branch) => {
            let mut out = branch.targets.clone();
            out.push(branch.default_label.clone());
            out
        }
        BlockPyTerm::Raise(_) | BlockPyTerm::Return(_) => Vec::new(),
    }
}

fn fresh_linearized_if_label(
    _base: &BlockPyLabel,
    counter: &mut usize,
    _suffix: &str,
) -> BlockPyLabel {
    let label = BlockPyLabel::from_index(*counter);
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

fn conservative_state_after_prefix<E, N>(
    base: &[String],
    body: &[StructuredBlockPyStmt<E, N>],
) -> Vec<String>
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
    body: Vec<StructuredBlockPyStmt<E, N>>,
    final_term: BlockPyTerm<E>,
    exc_edge: Option<super::BlockPyEdge>,
    block_params: Vec<String>,
    declared_params: Vec<BlockParam>,
    exc_target: Option<BlockPyLabel>,
    next_label_id: &mut usize,
    out_blocks: &mut Vec<CfgBlock<StructuredBlockPyStmt<E, N>, BlockPyTerm<E>>>,
    out_block_params: &mut HashMap<BlockPyLabel, Vec<String>>,
    out_exception_edges: &mut HashMap<BlockPyLabel, Option<BlockPyLabel>>,
) where
    E: Clone + Into<Expr>,
    N: BlockPyNameLike,
{
    let Some(if_index) = body
        .iter()
        .position(|stmt| matches!(stmt, StructuredBlockPyStmt::If(_)))
    else {
        out_block_params.insert(label.clone(), block_params.clone());
        out_exception_edges.insert(label.clone(), exc_target);
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
        Some(StructuredBlockPyStmt::If(if_stmt)) => if_stmt,
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

    out_block_params.insert(label.clone(), block_params);
    out_exception_edges.insert(label.clone(), exc_target.clone());
    out_blocks.push(CfgBlock {
        label: label.clone(),
        body,
        term: BlockPyTerm::IfTerm(BlockPyIfTerm {
            test: if_stmt.test.clone(),
            then_label: then_label.clone(),
            else_label: else_label.clone(),
        }),
        params: params_for_linearized_names(
            out_block_params.get(&label).unwrap(),
            &declared_params,
        ),
        exc_edge: exc_edge.clone(),
    });

    let branch_fallthrough = join_label
        .clone()
        .map(|next_label| BlockPyTerm::Jump(next_label.into()))
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
    fragment: BlockPyCfgFragment<StructuredBlockPyStmt<E, N>, BlockPyTerm<E>>,
    fallthrough_term: BlockPyTerm<E>,
    exc_edge: Option<super::BlockPyEdge>,
    block_params: Vec<String>,
    declared_params: Vec<BlockParam>,
    exc_target: Option<BlockPyLabel>,
    next_label_id: &mut usize,
    out_blocks: &mut Vec<CfgBlock<StructuredBlockPyStmt<E, N>, BlockPyTerm<E>>>,
    out_block_params: &mut HashMap<BlockPyLabel, Vec<String>>,
    out_exception_edges: &mut HashMap<BlockPyLabel, Option<BlockPyLabel>>,
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
    blocks: &[CfgBlock<StructuredBlockPyStmt<E, N>, BlockPyTerm<E>>],
    block_params: &HashMap<BlockPyLabel, Vec<String>>,
    exception_edges: &HashMap<BlockPyLabel, Option<BlockPyLabel>>,
) -> (
    Vec<CfgBlock<StructuredBlockPyStmt<E, N>, BlockPyTerm<E>>>,
    HashMap<BlockPyLabel, Vec<String>>,
    HashMap<BlockPyLabel, Option<BlockPyLabel>>,
)
where
    E: Clone + Into<Expr>,
    N: BlockPyNameLike,
{
    let mut out_blocks = Vec::new();
    let mut out_block_params = HashMap::new();
    let mut out_exception_edges = HashMap::new();
    let mut next_label_id = blocks
        .iter()
        .map(|block| block.label.index())
        .max()
        .map(|index| index + 1)
        .unwrap_or(0);
    for block in blocks {
        let mut params = block_params.get(&block.label).cloned().unwrap_or_default();
        for name in block.bb_param_names() {
            if !params.iter().any(|existing| existing == name) {
                params.push(name.to_string());
            }
        }
        let exc_target = exception_edges.get(&block.label).cloned().unwrap_or(None);
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
    blocks: &mut [CfgBlock<StructuredBlockPyStmt<E, N>, BlockPyTerm<E>>],
) where
    E: Clone + ImplicitNoneExpr,
    N: BlockPyNameLike,
{
    let trivial_ret_none_terms: HashMap<BlockPyLabel, BlockPyTerm<E>> = blocks
        .iter()
        .filter(|block| {
            block.body.is_empty()
                && match &block.term {
                    BlockPyTerm::Return(expr) => E::is_implicit_none_expr(expr),
                    _ => false,
                }
        })
        .map(|block| (block.label.clone(), block.term.clone()))
        .collect();

    for block in blocks.iter_mut() {
        let jump_target = match &block.term {
            BlockPyTerm::Jump(target) => Some(target.target.clone()),
            _ => None,
        };
        if let Some(target) = jump_target {
            if let Some(term) = trivial_ret_none_terms.get(&target) {
                block.term = term.clone();
            }
        }
    }
}

pub(crate) fn fold_constant_brif_blockpy(
    blocks: &mut [CfgBlock<StructuredBlockPyStmt<Expr>, BlockPyTerm<Expr>>],
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
                        Some(then_label.clone())
                    } else {
                        Some(else_label.clone())
                    }
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(target) = jump_target {
            block.term = BlockPyTerm::Jump(target.into());
        }
    }
}

pub(crate) fn prune_unreachable_blockpy_blocks<E>(
    entry_label: BlockPyLabel,
    extra_roots: &[BlockPyLabel],
    blocks: &mut Vec<CfgBlock<StructuredBlockPyStmt<E>, BlockPyTerm<E>>>,
) {
    let index_by_label: HashMap<BlockPyLabel, usize> = blocks
        .iter()
        .enumerate()
        .map(|(idx, block)| (block.label.clone(), idx))
        .collect();

    let mut worklist = vec![entry_label];
    worklist.extend(extra_roots.iter().cloned());
    let mut reachable = HashSet::new();
    while let Some(label) = worklist.pop() {
        if !reachable.insert(label.clone()) {
            continue;
        }
        let Some(idx) = index_by_label.get(&label) else {
            continue;
        };
        for succ in blockpy_successors(&blocks[*idx]) {
            worklist.push(succ);
        }
    }

    blocks.retain(|block| reachable.contains(&block.label));
}

pub(crate) fn relabel_blockpy_blocks_dense<S, T>(blocks: &mut [CfgBlock<S, T>])
where
    T: RelabelBlockTargets,
{
    let relabel = blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.label, BlockPyLabel::from_index(index)))
        .collect::<HashMap<_, _>>();

    for block in blocks.iter_mut() {
        block.label = relabel
            .get(&block.label)
            .expect("dense relabel should cover every block")
            .clone();
        block.term.relabel_targets(&relabel);
        if let Some(exc_edge) = &mut block.exc_edge {
            exc_edge.target = relabel
                .get(&exc_edge.target)
                .expect("dense relabel should cover every exception target")
                .clone();
        }
    }
}

pub(crate) trait RelabelBlockTargets {
    fn relabel_targets(&mut self, relabel: &HashMap<BlockPyLabel, BlockPyLabel>);
}

impl<E> RelabelBlockTargets for BlockPyTerm<E> {
    fn relabel_targets(&mut self, relabel: &HashMap<BlockPyLabel, BlockPyLabel>) {
        match self {
            BlockPyTerm::Jump(edge) => {
                edge.target = relabel
                    .get(&edge.target)
                    .expect("dense relabel should cover every jump target")
                    .clone();
            }
            BlockPyTerm::IfTerm(if_term) => {
                if_term.then_label = relabel
                    .get(&if_term.then_label)
                    .expect("dense relabel should cover every then target")
                    .clone();
                if_term.else_label = relabel
                    .get(&if_term.else_label)
                    .expect("dense relabel should cover every else target")
                    .clone();
            }
            BlockPyTerm::BranchTable(branch) => {
                for target in &mut branch.targets {
                    *target = relabel
                        .get(target)
                        .expect("dense relabel should cover every br_table target")
                        .clone();
                }
                branch.default_label = relabel
                    .get(&branch.default_label)
                    .expect("dense relabel should cover every br_table default target")
                    .clone();
            }
            BlockPyTerm::Raise(_) | BlockPyTerm::Return(_) => {}
        }
    }
}
