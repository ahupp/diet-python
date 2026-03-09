use super::dataflow::analyze_blockpy_use_def;
use super::symbol_analysis::{assigned_names_in_stmt, collect_assigned_names};
use crate::basic_block::block_py::{
    BlockPyBlock, BlockPyIf, BlockPyLegacyTryJump, BlockPyStmt, BlockPyTry,
};
use crate::basic_block::blockpy_to_bb;
use crate::basic_block::ruff_to_blockpy::lower_stmts_to_blockpy_stmts;
use crate::py_stmt;
use crate::transform::scope::cell_name;
use ruff_python_ast::{self as ast, Expr, Stmt};
use std::collections::{HashMap, HashSet};

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
    blocks: &[BlockPyBlock],
    module_init_mode: bool,
) -> Vec<String> {
    let mut defs_anywhere = HashSet::new();
    let mut injected_exception_names = HashSet::new();
    for block in blocks {
        for stmt in &block.body {
            defs_anywhere.extend(assigned_names_in_blockpy_stmt(stmt));
        }
        if let Some(BlockPyStmt::LegacyTryJump(BlockPyLegacyTryJump {
            except_exc_name,
            finally_exc_name,
            ..
        })) = block.body.last()
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
        let (uses, defs) = analyze_blockpy_use_def(block);
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

pub(super) fn collect_injected_exception_names_blockpy(blocks: &[BlockPyBlock]) -> HashSet<String> {
    let mut names = HashSet::new();
    for block in blocks {
        let Some(BlockPyStmt::LegacyTryJump(BlockPyLegacyTryJump {
            except_exc_name,
            finally_exc_name,
            ..
        })) = block.body.last()
        else {
            continue;
        };
        if let Some(name) = except_exc_name.as_ref() {
            names.insert(name.clone());
        }
        if let Some(name) = finally_exc_name.as_ref() {
            names.insert(name.clone());
        }
    }
    names
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

pub(crate) fn sync_target_cells_stmts(target: &Expr, cell_slots: &HashSet<String>) -> Vec<Stmt> {
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

fn is_generator_dispatch_param(name: &str) -> bool {
    matches!(
        name,
        "_dp_self" | "_dp_send_value" | "_dp_resume_exc" | "_dp_transport_sent"
    )
}

fn sync_generator_storage_name(name: &str) -> String {
    if name == "_dp_classcell" || name.starts_with("_dp_cell_") {
        return name.to_string();
    }
    cell_name(name)
}

pub(super) fn sync_generator_cleanup_cells(
    state_vars: &[String],
    injected_exception_names: &HashSet<String>,
) -> Vec<String> {
    sync_generator_state_order(state_vars, injected_exception_names)
        .into_iter()
        .filter(|name| {
            name != "_dp_pc" && name != "_dp_classcell" && !name.starts_with("_dp_cell_")
        })
        .map(|name| sync_generator_storage_name(name.as_str()))
        .collect()
}

fn assigned_names_in_blockpy_stmt(stmt: &BlockPyStmt) -> HashSet<String> {
    match stmt {
        BlockPyStmt::Assign(assign) => HashSet::from([assign.target.id.to_string()]),
        BlockPyStmt::FunctionDef(func_def) => HashSet::from([func_def.name.id.to_string()]),
        other => {
            let Some(stmt) = blockpy_to_bb::blockpy_stmt_to_stmt_for_analysis(other) else {
                return HashSet::new();
            };
            assigned_names_in_stmt(&stmt)
        }
    }
}

pub(super) fn sync_generator_state_order(
    state_vars: &[String],
    injected_exception_names: &HashSet<String>,
) -> Vec<String> {
    let _ = injected_exception_names;
    state_vars
        .iter()
        .filter(|name| !is_generator_dispatch_param(name.as_str()))
        .cloned()
        .collect()
}

pub(super) fn rewrite_sync_generator_blockpy_blocks_to_closure_cells(
    blocks: &mut [BlockPyBlock],
    block_params: &mut HashMap<String, Vec<String>>,
    state_vars: &[String],
    cell_slots: &mut HashSet<String>,
    _entry_label: &str,
) {
    let injected_exception_names = collect_injected_exception_names_blockpy(blocks);
    let lifted_state = sync_generator_state_order(state_vars, &injected_exception_names);
    let passthrough_exception_names = state_vars
        .iter()
        .filter(|name| injected_exception_names.contains(name.as_str()))
        .cloned()
        .collect::<HashSet<_>>();
    let lifted_cells = lifted_state
        .iter()
        .map(|name| sync_generator_storage_name(name))
        .collect::<Vec<_>>();
    let lifted_storage_names = lifted_cells.iter().cloned().collect::<HashSet<_>>();
    for (name, cell) in lifted_state.iter().zip(lifted_cells.iter()) {
        if name == "_dp_classcell" || name.starts_with("_dp_cell_") {
            continue;
        }
        cell_slots.insert(cell.clone());
    }

    for block in blocks.iter_mut() {
        rewrite_sync_generator_blockpy_block(
            block,
            block_params,
            &passthrough_exception_names,
            &lifted_state,
            &lifted_storage_names,
            &injected_exception_names,
            cell_slots,
        );
    }
}

fn rewrite_sync_generator_blockpy_block(
    block: &mut BlockPyBlock,
    block_params: &mut HashMap<String, Vec<String>>,
    passthrough_exception_names: &HashSet<String>,
    lifted_state: &[String],
    lifted_storage_names: &HashSet<String>,
    injected_exception_names: &HashSet<String>,
    cell_slots: &HashSet<String>,
) {
    {
        let (uses_before_def, _) = analyze_blockpy_use_def(block);
        let mut preload = Vec::new();
        for name in passthrough_exception_names {
            if !params_contain(block_params, block.label.as_str(), name.as_str()) {
                continue;
            }
            let cell = sync_generator_storage_name(name);
            preload.extend(lower_generated_stmts_to_blockpy(vec![py_stmt!(
                "__dp_store_cell_if_not_deleted({cell:id}, {name:id})",
                name = name.as_str(),
                cell = cell.as_str(),
            )]));
        }
        for name in lifted_state {
            if name.starts_with("_dp_cell_") || name == "_dp_classcell" {
                continue;
            }
            if !uses_before_def.contains(name.as_str()) {
                continue;
            }
            let cell = sync_generator_storage_name(name);
            preload.extend(lower_generated_stmts_to_blockpy(vec![py_stmt!(
                "{name:id} = __dp_load_deleted_name({display_name:literal}, __dp_load_cell({cell:id}))",
                name = name.as_str(),
                display_name = name.as_str(),
                cell = cell.as_str(),
            )]));
        }
        if !preload.is_empty() {
            let mut new_body = preload;
            new_body.extend(std::mem::take(&mut block.body));
            block.body = new_body;
        }

        let mut new_body = Vec::with_capacity(block.body.len());
        for stmt in std::mem::take(&mut block.body) {
            let sync_stmts = match &stmt {
                BlockPyStmt::Assign(assign) => {
                    sync_target_cells_stmts(&Expr::Name(assign.target.clone()), cell_slots)
                }
                _ => Vec::new(),
            };
            new_body.push(stmt);
            new_body.extend(lower_generated_stmts_to_blockpy(sync_stmts));
        }
        block.body = new_body;

        let params = block_params
            .entry(block.label.as_str().to_string())
            .or_default();
        let has_self = params.iter().any(|name| name == "_dp_self");
        let has_send = params.iter().any(|name| name == "_dp_send_value");
        let has_exc = params.iter().any(|name| name == "_dp_resume_exc");
        let has_transport = params.iter().any(|name| name == "_dp_transport_sent");
        let mut rewritten = Vec::new();
        if has_self {
            rewritten.push("_dp_self".to_string());
        }
        if has_send {
            rewritten.push("_dp_send_value".to_string());
        }
        if has_exc {
            rewritten.push("_dp_resume_exc".to_string());
        }
        if has_transport {
            rewritten.push("_dp_transport_sent".to_string());
        }
        for name in params.iter() {
            if is_generator_dispatch_param(name.as_str()) {
                continue;
            }
            if name.starts_with("_dp_try_exc_") || injected_exception_names.contains(name.as_str())
            {
                if !rewritten.iter().any(|existing| existing == name) {
                    rewritten.push(name.clone());
                }
                continue;
            }
            let rewritten_name = sync_generator_storage_name(name);
            if lifted_storage_names.contains(rewritten_name.as_str()) {
                continue;
            }
            if !rewritten.iter().any(|existing| existing == &rewritten_name) {
                rewritten.push(rewritten_name);
            }
        }
        *params = rewritten;
    }

    for stmt in &mut block.body {
        match stmt {
            BlockPyStmt::If(BlockPyIf { body, orelse, .. }) => {
                for nested in body {
                    rewrite_sync_generator_blockpy_block(
                        nested,
                        block_params,
                        passthrough_exception_names,
                        lifted_state,
                        lifted_storage_names,
                        injected_exception_names,
                        cell_slots,
                    );
                }
                for nested in orelse {
                    rewrite_sync_generator_blockpy_block(
                        nested,
                        block_params,
                        passthrough_exception_names,
                        lifted_state,
                        lifted_storage_names,
                        injected_exception_names,
                        cell_slots,
                    );
                }
            }
            BlockPyStmt::Try(BlockPyTry {
                body,
                handlers,
                orelse,
                finalbody,
            }) => {
                for nested in body {
                    rewrite_sync_generator_blockpy_block(
                        nested,
                        block_params,
                        passthrough_exception_names,
                        lifted_state,
                        lifted_storage_names,
                        injected_exception_names,
                        cell_slots,
                    );
                }
                for handler in handlers {
                    for nested in &mut handler.body {
                        rewrite_sync_generator_blockpy_block(
                            nested,
                            block_params,
                            passthrough_exception_names,
                            lifted_state,
                            lifted_storage_names,
                            injected_exception_names,
                            cell_slots,
                        );
                    }
                }
                for nested in orelse {
                    rewrite_sync_generator_blockpy_block(
                        nested,
                        block_params,
                        passthrough_exception_names,
                        lifted_state,
                        lifted_storage_names,
                        injected_exception_names,
                        cell_slots,
                    );
                }
                for nested in finalbody {
                    rewrite_sync_generator_blockpy_block(
                        nested,
                        block_params,
                        passthrough_exception_names,
                        lifted_state,
                        lifted_storage_names,
                        injected_exception_names,
                        cell_slots,
                    );
                }
            }
            _ => {}
        }
    }
}

fn params_contain(block_params: &HashMap<String, Vec<String>>, label: &str, name: &str) -> bool {
    block_params
        .get(label)
        .map(|params| params.iter().any(|param| param == name))
        .unwrap_or(false)
}

fn lower_generated_stmts_to_blockpy(stmts: Vec<Stmt>) -> Vec<BlockPyStmt> {
    lower_stmts_to_blockpy_stmts(&stmts)
        .unwrap_or_else(|err| panic!("failed to convert generated stmt to BlockPy: {err}"))
}
