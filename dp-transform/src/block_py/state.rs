use super::dataflow::{
    analyze_blockpy_use_def, assigned_names_in_blockpy_stmt, assigned_names_in_blockpy_term,
};
use super::{CfgBlock, Expr, IntoBlockPyStmt, IntoBlockPyTerm};
use crate::passes::ast_symbol_analysis::{assigned_names_in_stmt, collect_assigned_names};
use crate::passes::ast_to_ast::scope::cell_name;
use crate::py_stmt;
use ruff_python_ast::{self as ast, Stmt};
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

pub(crate) fn collect_state_vars<S, T, E, M>(
    param_names: &[String],
    blocks: &[CfgBlock<S, T, M>],
) -> Vec<String>
where
    S: IntoBlockPyStmt<E>,
    T: IntoBlockPyTerm<E>,
    E: Clone + Into<Expr>,
{
    let mut defs_anywhere = HashSet::new();
    for block in blocks {
        for stmt in &block.body {
            let stmt = stmt.clone().into_stmt();
            defs_anywhere.extend(assigned_names_in_blockpy_stmt(&stmt));
        }
        let term = block.term.clone().into_term();
        defs_anywhere.extend(assigned_names_in_blockpy_term(&term));
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

pub(crate) fn collect_cell_slots(stmts: &[Stmt]) -> HashSet<String> {
    let mut slots = HashSet::new();
    for stmt in stmts {
        let mut names: HashSet<String> = assigned_names_in_stmt(stmt);
        for name in names.drain() {
            if name.starts_with("_dp_cell_") {
                slots.insert(name);
            }
        }
    }
    slots
}

pub(crate) fn sync_target_cells_stmts(target: &Expr, cell_slots: &HashSet<String>) -> Vec<Stmt> {
    let mut names: HashSet<String> = HashSet::new();
    collect_assigned_names(target, &mut names);
    let mut names = names.into_iter().collect::<Vec<_>>();
    names.sort();

    names
        .into_iter()
        .filter_map(|name| {
            let cell = cell_name(name.as_str());
            if !cell_slots.contains(cell.as_str()) {
                return None;
            }
            Some(py_stmt!(
                "__dp_store_cell({cell:id}, {value:id})",
                cell = cell.as_str(),
                value = name.as_str(),
            ))
        })
        .collect()
}
