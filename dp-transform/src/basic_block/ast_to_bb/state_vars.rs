use super::dataflow::analyze_block_use_def;
use super::symbol_analysis::{assigned_names_in_stmt, collect_assigned_names};
use super::{Block, Terminator};
use crate::transform::scope::cell_name;
use crate::py_stmt;
use ruff_python_ast::{self as ast, Expr, Stmt};
use std::collections::HashSet;

pub(super) fn collect_parameter_names(parameters: &ast::Parameters) -> Vec<String> {
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

pub(super) fn collect_state_vars(
    param_names: &[String],
    blocks: &[Block],
    module_init_mode: bool,
) -> Vec<String> {
    let mut defs_anywhere = HashSet::new();
    let mut injected_exception_names = HashSet::new();
    for block in blocks {
        for stmt in &block.body {
            defs_anywhere.extend(assigned_names_in_stmt(stmt));
        }
        if let Terminator::TryJump {
            except_exc_name,
            finally_exc_name,
            ..
        } = &block.terminator
        {
            if let Some(name) = except_exc_name.as_ref() {
                injected_exception_names.insert(name.clone());
            }
            if let Some(name) = finally_exc_name.as_ref() {
                injected_exception_names.insert(name.clone());
            }
        }
    }

    let mut state = param_names.to_vec();
    for block in blocks {
        let (uses, defs) = analyze_block_use_def(block);
        let mut names = defs.into_iter().collect::<Vec<_>>();
        for name in uses {
            let is_special_runtime_state = name == "_dp_self"
                || name.starts_with("_dp_cell_")
                || name.starts_with("_dp_try_exc_")
                || name == "_dp_classcell";
            let is_known_local = defs_anywhere.contains(name.as_str())
                || injected_exception_names.contains(name.as_str())
                || param_names.iter().any(|param| param == &name);
            let include = if module_init_mode {
                is_special_runtime_state || is_known_local
            } else {
                is_special_runtime_state || is_known_local
            };
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

pub(super) fn collect_cell_slots(stmts: &[Box<Stmt>]) -> HashSet<String> {
    let mut slots = HashSet::new();
    for stmt in stmts {
        let mut names = assigned_names_in_stmt(stmt.as_ref());
        for name in names.drain() {
            if name.starts_with("_dp_cell_") {
                slots.insert(name);
            }
        }
    }
    slots
}

pub(super) fn sync_target_cells_stmts(target: &Expr, cell_slots: &HashSet<String>) -> Vec<Stmt> {
    let mut names = HashSet::new();
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
