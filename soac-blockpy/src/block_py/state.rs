use super::dataflow::{assigned_names_in_blockpy_term, assigned_names_in_linear_blockpy_stmt};
use super::{BlockPyNameLike, BlockPySemanticExprNode, BlockPyStmt, BlockPyTerm, CfgBlock, Instr};

pub(crate) fn collect_state_vars<E, N>(
    param_names: &[String],
    blocks: &[CfgBlock<BlockPyStmt<E, N>, BlockPyTerm<E>>],
) -> Vec<String>
where
    E: BlockPySemanticExprNode + Instr,
    N: BlockPyNameLike,
{
    let mut state = param_names.to_vec();
    for block in blocks {
        for param_name in block
            .exception_param()
            .into_iter()
            .chain(block.param_names())
        {
            if !state.iter().any(|existing| existing == param_name) {
                state.push(param_name.to_string());
            }
        }
        for stmt in &block.body {
            for name in assigned_names_in_linear_blockpy_stmt(stmt) {
                if !state.iter().any(|existing| existing == &name) {
                    state.push(name);
                }
            }
        }
        for name in assigned_names_in_blockpy_term(&block.term) {
            if !state.iter().any(|existing| existing == &name) {
                state.push(name);
            }
        }
    }
    state
}
