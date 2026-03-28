use super::dataflow::{assigned_names_in_blockpy_stmt, assigned_names_in_blockpy_term};
use super::{BlockPyNameLike, BlockPyTerm, CfgBlock, Expr, IntoBlockPyStmt};

pub(crate) fn collect_state_vars<S, E, N>(
    param_names: &[String],
    blocks: &[CfgBlock<S, BlockPyTerm<E>>],
) -> Vec<String>
where
    S: IntoBlockPyStmt<E, N>,
    E: Clone + Into<Expr>,
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
            let stmt = stmt.clone().into_stmt();
            for name in assigned_names_in_blockpy_stmt(&stmt) {
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
