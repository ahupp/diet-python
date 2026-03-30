use crate::block_py::{
    BlockPyFunction, BlockPyLabel, BlockPyModule, BlockPyStmt, BlockPyTerm, ResolvedStorageBlock,
};
use crate::passes::ruff_to_blockpy::populate_exception_edge_args;
use crate::passes::ResolvedStorageBlockPyPass;
use std::collections::HashSet;

pub fn lower_try_jump_exception_flow(
    module: &BlockPyModule<ResolvedStorageBlockPyPass>,
) -> BlockPyModule<ResolvedStorageBlockPyPass> {
    let callable_defs = module
        .callable_defs
        .iter()
        .cloned()
        .map(lower_function_try_jump_exception_flow)
        .collect();
    BlockPyModule { callable_defs }
}

fn lower_function_try_jump_exception_flow(
    function: BlockPyFunction<ResolvedStorageBlockPyPass>,
) -> BlockPyFunction<ResolvedStorageBlockPyPass> {
    let mut function = BlockPyFunction {
        function_id: function.function_id,
        name_gen: function.name_gen,
        names: function.names,
        kind: function.kind,
        params: function.params,
        blocks: function.blocks,
        doc: function.doc,
        storage_layout: function.storage_layout,
        semantic: function.semantic,
    };
    // Canonicalize exception-edge blocks so each potentially-raising expression
    // step sits in its own block. This keeps per-expression exception checks
    // explicit in CFG shape (op-block -> jump -> ... -> term-block), which
    // allows the JIT fast path to dispatch exceptions directly from each step.
    split_exception_blocks_for_expr_checks(&mut function);
    populate_exception_edge_args(&mut function.blocks);

    function
}

fn bb_params_from_names(
    param_names: Vec<String>,
    exception_name: Option<&str>,
) -> Vec<crate::block_py::BlockParam> {
    param_names
        .into_iter()
        .map(|name| crate::block_py::BlockParam {
            role: if exception_name == Some(name.as_str()) {
                crate::block_py::BlockParamRole::Exception
            } else {
                crate::block_py::BlockParamRole::Local
            },
            name,
        })
        .collect()
}

fn split_exception_blocks_for_expr_checks(
    function: &mut BlockPyFunction<ResolvedStorageBlockPyPass>,
) {
    let mut used_labels: HashSet<BlockPyLabel> =
        function.blocks.iter().map(|block| block.label).collect();
    let mut fresh_index: usize = 0;
    let mut out = Vec::with_capacity(function.blocks.len());

    for block in std::mem::take(&mut function.blocks) {
        if block.exc_edge.is_none() || block.body.is_empty() {
            out.push(block);
            continue;
        }

        let mut known_names = block.param_name_vec();
        let mut current_label = block.label;
        let exc_edge = block.exc_edge.clone();
        let edge_exc_name = block.exception_param().map(ToString::to_string);
        let mut ops = block.body.into_iter().peekable();
        let mut segment_start_names = known_names.clone();

        let mut segment_ops: Vec<BlockPyStmt> = Vec::new();
        while let Some(op) = ops.next() {
            let ends_segment = op_updates_exception_state(&op) && ops.peek().is_some();
            segment_ops.push(op.clone());
            apply_op_effect_to_known_names(&op, &mut known_names);

            if ends_segment {
                let next_label = unique_exc_split_label(&mut used_labels, fresh_index);
                fresh_index += 1;
                out.push(ResolvedStorageBlock {
                    label: current_label,
                    body: std::mem::take(&mut segment_ops),
                    term: BlockPyTerm::Jump(next_label.into()),
                    params: bb_params_from_names(
                        segment_start_names.clone(),
                        edge_exc_name.as_deref(),
                    ),
                    exc_edge: exc_edge.clone(),
                });
                current_label = next_label;
                segment_start_names = known_names.clone();
            }

            if ops.peek().is_none() {
                out.push(ResolvedStorageBlock {
                    label: current_label,
                    body: std::mem::take(&mut segment_ops),
                    term: block.term.clone(),
                    params: bb_params_from_names(
                        segment_start_names.clone(),
                        edge_exc_name.as_deref(),
                    ),
                    exc_edge: exc_edge.clone(),
                });
            }
        }
    }

    function.blocks = out;
}

fn op_updates_exception_state(op: &BlockPyStmt) -> bool {
    matches!(op, BlockPyStmt::Assign(_) | BlockPyStmt::Delete(_))
}

fn unique_exc_split_label(
    used_labels: &mut HashSet<BlockPyLabel>,
    index_seed: usize,
) -> BlockPyLabel {
    let mut index = index_seed;
    loop {
        let candidate = BlockPyLabel::from_index(index);
        if used_labels.insert(candidate) {
            return candidate;
        }
        index += 1;
    }
}

fn apply_op_effect_to_known_names(op: &BlockPyStmt, known_names: &mut Vec<String>) {
    match op {
        BlockPyStmt::Assign(assign) => {
            let target = assign.target.id.to_string();
            for target_name in [Some(target.as_str()), target.strip_prefix("_dp_cell_")]
                .into_iter()
                .flatten()
            {
                if !known_names.iter().any(|name| name == target_name) {
                    known_names.push(target_name.to_string());
                }
            }
        }
        BlockPyStmt::Expr(_) => {}
        BlockPyStmt::Delete(delete) => {
            let target_name = delete.target.id.to_string();
            known_names.retain(|existing| {
                existing != &target_name
                    && target_name
                        .strip_prefix("_dp_cell_")
                        .map(|logical_name| existing != logical_name)
                        .unwrap_or(true)
            });
        }
    }
}

#[cfg(test)]
mod test;
