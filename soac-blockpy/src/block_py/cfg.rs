use super::{
    BlockParam, BlockPyCfgFragment, BlockPyIfTerm, BlockPyLabel, BlockPyTerm, CfgBlock,
    FunctionNameGen, ImplicitNoneExpr, Instr, StructuredInstr,
};
use ruff_python_ast::Expr;
use std::collections::{HashMap, HashSet};

fn blockpy_successors<E: Instr>(
    block: &CfgBlock<StructuredInstr<E>, BlockPyTerm<E>>,
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

fn params_for_linearized_names(
    param_names: &[String],
    declared_params: &[BlockParam],
) -> Vec<BlockParam> {
    declared_params
        .iter()
        .filter(|param| param_names.iter().any(|name| name == &param.name))
        .cloned()
        .collect()
}

fn linearize_blockpy_if_sequence<E>(
    name_gen: &FunctionNameGen,
    label: BlockPyLabel,
    body: Vec<StructuredInstr<E>>,
    final_term: BlockPyTerm<E>,
    exc_edge: Option<super::BlockPyEdge>,
    block_params: Vec<String>,
    declared_params: Vec<BlockParam>,
    exc_target: Option<BlockPyLabel>,
    out_blocks: &mut Vec<CfgBlock<StructuredInstr<E>, BlockPyTerm<E>>>,
    out_block_params: &mut HashMap<BlockPyLabel, Vec<String>>,
    out_exception_edges: &mut HashMap<BlockPyLabel, Option<BlockPyLabel>>,
) where
    E: Clone + Instr,
{
    let Some(if_index) = body
        .iter()
        .position(|stmt| matches!(stmt, StructuredInstr::If(_)))
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
        Some(StructuredInstr::If(if_stmt)) => if_stmt,
        _ => unreachable!("expected structured BlockPy if at split point"),
    };
    let then_label = name_gen.next_block_name();
    let else_label = name_gen.next_block_name();
    let join_label = if rest.is_empty() {
        None
    } else {
        Some(name_gen.next_block_name())
    };

    out_block_params.insert(label.clone(), block_params.clone());
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
        .map(|next_label| BlockPyTerm::Jump(super::BlockPyEdge::new(next_label)))
        .unwrap_or_else(|| final_term.clone());
    linearize_blockpy_fragment(
        name_gen,
        then_label,
        if_stmt.body,
        branch_fallthrough.clone(),
        exc_edge.clone(),
        block_params.clone(),
        declared_params.clone(),
        exc_target.clone(),
        out_blocks,
        out_block_params,
        out_exception_edges,
    );
    linearize_blockpy_fragment(
        name_gen,
        else_label,
        if_stmt.orelse,
        branch_fallthrough,
        exc_edge.clone(),
        block_params.clone(),
        declared_params.clone(),
        exc_target.clone(),
        out_blocks,
        out_block_params,
        out_exception_edges,
    );

    if let Some(join_label) = join_label {
        linearize_blockpy_if_sequence(
            name_gen,
            join_label,
            rest,
            final_term,
            exc_edge,
            block_params,
            declared_params,
            exc_target,
            out_blocks,
            out_block_params,
            out_exception_edges,
        );
    }
}

fn linearize_blockpy_fragment<E>(
    name_gen: &FunctionNameGen,
    label: BlockPyLabel,
    fragment: BlockPyCfgFragment<StructuredInstr<E>, BlockPyTerm<E>>,
    fallthrough_term: BlockPyTerm<E>,
    exc_edge: Option<super::BlockPyEdge>,
    block_params: Vec<String>,
    declared_params: Vec<BlockParam>,
    exc_target: Option<BlockPyLabel>,
    out_blocks: &mut Vec<CfgBlock<StructuredInstr<E>, BlockPyTerm<E>>>,
    out_block_params: &mut HashMap<BlockPyLabel, Vec<String>>,
    out_exception_edges: &mut HashMap<BlockPyLabel, Option<BlockPyLabel>>,
) where
    E: Clone + Instr,
{
    linearize_blockpy_if_sequence(
        name_gen,
        label,
        fragment.body,
        fragment.term.unwrap_or(fallthrough_term),
        exc_edge,
        block_params,
        declared_params,
        exc_target,
        out_blocks,
        out_block_params,
        out_exception_edges,
    );
}

pub(crate) fn linearize_structured_ifs<E>(
    name_gen: &FunctionNameGen,
    blocks: &[CfgBlock<StructuredInstr<E>, BlockPyTerm<E>>],
    block_params: &HashMap<BlockPyLabel, Vec<String>>,
    exception_edges: &HashMap<BlockPyLabel, Option<BlockPyLabel>>,
) -> (
    Vec<CfgBlock<StructuredInstr<E>, BlockPyTerm<E>>>,
    HashMap<BlockPyLabel, Vec<String>>,
    HashMap<BlockPyLabel, Option<BlockPyLabel>>,
)
where
    E: Clone + Instr,
{
    let mut out_blocks = Vec::new();
    let mut out_block_params = HashMap::new();
    let mut out_exception_edges = HashMap::new();
    for block in blocks {
        let mut params = block_params.get(&block.label).cloned().unwrap_or_default();
        for name in block.bb_param_names() {
            if !params.iter().any(|existing| existing == name) {
                params.push(name.to_string());
            }
        }
        let exc_target = exception_edges.get(&block.label).cloned().unwrap_or(None);
        linearize_blockpy_if_sequence(
            name_gen,
            block.label.clone(),
            block.body.clone(),
            block.term.clone(),
            block.exc_edge.clone(),
            params,
            block.params.clone(),
            exc_target,
            &mut out_blocks,
            &mut out_block_params,
            &mut out_exception_edges,
        );
    }
    (out_blocks, out_block_params, out_exception_edges)
}

pub(crate) fn fold_jumps_to_trivial_none_return_blockpy<E>(
    blocks: &mut [CfgBlock<StructuredInstr<E>, BlockPyTerm<E>>],
) where
    E: Clone + ImplicitNoneExpr + Instr,
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
    blocks: &mut [CfgBlock<StructuredInstr<Expr>, BlockPyTerm<Expr>>],
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
            block.term = BlockPyTerm::Jump(super::BlockPyEdge::new(target));
        }
    }
}

pub(crate) fn prune_unreachable_blockpy_blocks<E: Instr>(
    entry_label: BlockPyLabel,
    extra_roots: &[BlockPyLabel],
    blocks: &mut Vec<CfgBlock<StructuredInstr<E>, BlockPyTerm<E>>>,
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
