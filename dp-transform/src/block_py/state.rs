use super::dataflow::{
    analyze_blockpy_use_def, assigned_names_in_blockpy_stmt, assigned_names_in_blockpy_term,
};
use super::{BlockPyTerm, CfgBlock, Expr, IntoBlockPyStmt};
use ruff_python_ast as ast;
use std::collections::HashSet;

pub(crate) fn collect_parameter_names(parameters: &ast::Parameters) -> Vec<String> {
    let mut names = Vec::new();
    for param in &parameters.posonlyargs {
        names.push(param.parameter.name.id.to_string());
    }
    for param in &parameters.args {
        names.push(param.parameter.name.id.to_string());
    }
    if let Some(vararg) = &parameters.vararg {
        names.push(vararg.name.id.to_string());
    }
    for param in &parameters.kwonlyargs {
        names.push(param.parameter.name.id.to_string());
    }
    if let Some(kwarg) = &parameters.kwarg {
        names.push(kwarg.name.id.to_string());
    }
    names
}

pub(crate) fn collect_state_vars<S, E>(
    param_names: &[String],
    blocks: &[CfgBlock<S, BlockPyTerm<E>>],
) -> Vec<String>
where
    S: IntoBlockPyStmt<E>,
    E: Clone + Into<Expr>,
{
    let mut defs_anywhere = HashSet::new();
    for block in blocks {
        for stmt in &block.body {
            let stmt = stmt.clone().into_stmt();
            defs_anywhere.extend(assigned_names_in_blockpy_stmt(&stmt));
        }
        defs_anywhere.extend(assigned_names_in_blockpy_term(&block.term));
    }

    let mut state = param_names.to_vec();
    for block in blocks {
        let (uses, defs) = analyze_blockpy_use_def(block);
        let mut names = defs.into_iter().collect::<Vec<_>>();
        for name in uses {
            let is_special_runtime_state = name == "_dp_self"
                || name.starts_with("_dp_cell_")
                || name.starts_with("_dp_try_exc_")
                || name == "_dp_classcell";
            let is_known_local = defs_anywhere.contains(name.as_str())
                || param_names.iter().any(|param| param == &name);
            let include = is_special_runtime_state || is_known_local;
            if include {
                names.push(name);
            }
        }
        names.sort();
        names.dedup();
        for name in names {
            if !state.iter().any(|existing| existing == &name) {
                state.push(name);
            }
        }
    }
    state
}
