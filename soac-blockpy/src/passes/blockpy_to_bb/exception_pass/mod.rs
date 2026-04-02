use crate::block_py::{
    BlockPyFunction, BlockPyLabel, BlockPyModule, BlockPyStmt, BlockPyTerm, CoreBlockPyExpr,
    ResolvedStorageBlock,
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
    BlockPyModule {
        callable_defs,
        module_constants: module.module_constants.clone(),
    }
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

        let segment_params = block.bb_params().cloned().collect::<Vec<_>>();
        let mut current_label = block.label;
        let exc_edge = block.exc_edge.clone();
        let mut ops = block.body.into_iter().peekable();

        let mut segment_ops: Vec<BlockPyStmt> = Vec::new();
        while let Some(op) = ops.next() {
            let ends_segment = op_updates_exception_state(&op) && ops.peek().is_some();
            segment_ops.push(op.clone());

            if ends_segment {
                let next_label = unique_exc_split_label(&mut used_labels, fresh_index);
                fresh_index += 1;
                out.push(ResolvedStorageBlock {
                    label: current_label,
                    body: std::mem::take(&mut segment_ops),
                    term: BlockPyTerm::Jump(next_label.into()),
                    params: segment_params.clone(),
                    exc_edge: exc_edge.clone(),
                });
                current_label = next_label;
            }

            if ops.peek().is_none() {
                out.push(ResolvedStorageBlock {
                    label: current_label,
                    body: std::mem::take(&mut segment_ops),
                    term: block.term.clone(),
                    params: segment_params.clone(),
                    exc_edge: exc_edge.clone(),
                });
            }
        }
    }

    function.blocks = out;
}

fn op_updates_exception_state(op: &BlockPyStmt) -> bool {
    matches!(
        op,
        BlockPyStmt::Expr(CoreBlockPyExpr::Store(_)) | BlockPyStmt::Expr(CoreBlockPyExpr::Del(_))
    )
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

#[cfg(test)]
mod test;
