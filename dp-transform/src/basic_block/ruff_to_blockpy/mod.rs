use super::await_lower::{
    coroutine_generator_marker_stmt, lower_coroutine_awaits_in_stmt,
    lower_coroutine_awaits_in_stmts, lower_coroutine_awaits_to_yield_from,
};
use super::bb_ir::{BbClosureLayout, BbExpr, BbFunctionKind, BindingTarget};
use super::block_py::cfg::{
    fold_constant_brif_blockpy, fold_jumps_to_trivial_none_return_blockpy,
    prune_unreachable_blockpy_blocks, relabel_blockpy_blocks,
};
use super::block_py::dataflow::compute_block_params_blockpy;
use super::block_py::exception::{
    contains_return_stmt_in_body, contains_return_stmt_in_handlers,
    rewrite_region_returns_to_finally_blockpy,
};
use super::block_py::state::{
    collect_cell_slots, collect_injected_exception_names_blockpy, collect_state_vars,
    rewrite_sync_generator_blockpy_blocks_to_closure_cells, sync_generator_cleanup_cells,
    sync_generator_state_order, sync_target_cells_stmts as sync_target_cells_stmts_shared,
};
use super::block_py::{
    BlockPyAssign, BlockPyBlock, BlockPyDelete, BlockPyExpr, BlockPyFunction, BlockPyFunctionKind,
    BlockPyIf, BlockPyIfTerm, BlockPyLabel, BlockPyModule, BlockPyRaise, BlockPyStmt, BlockPyTerm,
    BlockPyTryJump,
};
use super::stmt_utils::flatten_stmt_boxes;
use crate::basic_block::ast_to_ast::ast_rewrite::Rewrite;
use crate::basic_block::ast_to_ast::rewrite_expr::make_tuple;
use crate::basic_block::ast_to_ast::rewrite_stmt;
use crate::namegen::fresh_name;
use crate::ruff_ast_to_string;
use crate::template::{empty_body, into_body, is_simple};
use crate::transformer::walk_stmt;
use crate::transformer::{walk_expr, Transformer};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt, StmtBody};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};

mod compat;
mod generator_lowering;
mod stmt_lowering;
mod stmt_sequences;
mod try_regions;

pub(crate) use generator_lowering::{
    blockpy_stmt_requires_generator_rest_entry, build_async_for_continue_entry,
    build_blockpy_closure_layout, build_closure_backed_generator_export_plan,
    build_initial_generator_metadata, lower_generator_blockpy_stmt_in_sequence,
    lower_generator_yield_terms_to_explicit_return_blockpy,
    split_generator_return_terms_to_escape_blocks, synthesize_generator_dispatch_metadata,
    GeneratorMetadata, GeneratorYieldSite,
};

pub(crate) use compat::{
    compat_block_from_blockpy, compat_if_jump_block, compat_jump_block_from_blockpy,
    compat_next_label, compat_next_temp, compat_raise_block_from_blockpy_raise,
    compat_return_block_from_expr, compat_sanitize_ident, emit_for_loop_blocks,
    emit_if_branch_block, emit_sequence_jump_block, emit_sequence_raise_block,
    emit_sequence_return_block, emit_simple_while_blocks, finalize_blockpy_block,
    lower_for_loop_continue_entry_with_state,
};
pub(crate) use stmt_lowering::{
    build_for_target_assign_body, desugar_structured_with_stmt_for_blockpy,
    lower_body_to_blocks_with_entry, lower_generated_stmts_into_blockpy,
    lower_nested_body_to_stmts, lower_orelse_to_stmts, lower_star_try_stmt_sequence,
    lower_stmt_into, lower_try_stmt_sequence, lower_with_stmt_sequence,
};
pub(crate) use stmt_sequences::{
    drive_stmt_sequence_until_control, lower_common_stmt_sequence_head,
    lower_expanded_stmt_sequence, lower_for_stmt_body_entry, lower_for_stmt_exit_entries,
    lower_for_stmt_sequence, lower_for_stmt_sequence_head, lower_generator_stmt_sequence_head,
    lower_generator_stmt_sequence_plan, lower_if_stmt_sequence, lower_if_stmt_sequence_from_stmt,
    lower_stmt_sequence_with_state, lower_stmts_to_blockpy_stmts, lower_top_level_function,
    lower_while_stmt_sequence, lower_while_stmt_sequence_from_stmt,
    plan_generator_stmt_in_sequence, plan_stmt_sequence_head, GeneratorStmtSequencePlan,
};
pub(crate) use try_regions::{
    block_references_label, build_try_plan, collect_region_label_names,
    emit_finally_return_dispatch_blocks, emit_try_jump_entry, finalize_try_regions,
    lower_try_regions, prepare_except_body, prepare_finally_body, LoweredTryRegions, TryPlan,
};

pub(crate) struct BlockPySequenceGeneratorState {
    pub enabled: bool,
    pub closure_state: bool,
    pub resume_order: Vec<String>,
    pub yield_sites: Vec<GeneratorYieldSite>,
}

pub(crate) struct GeneratorStmtSequenceLoweringState {
    pub enabled: bool,
    pub closure_state: bool,
    pub resume_order: Vec<String>,
    pub yield_sites: Vec<GeneratorYieldSite>,
    pub next_block_id: usize,
}

pub(crate) struct LoweredBlockPyFunction {
    pub function: BlockPyFunction,
    pub is_coroutine: bool,
    pub bb_kind: BbFunctionKind,
    pub block_params: HashMap<String, Vec<String>>,
    pub exception_edges: HashMap<String, Option<String>>,
    pub closure_layout: Option<BbClosureLayout>,
    pub param_specs: BbExpr,
}

pub(crate) struct LoweredBlockPyFunctionBundle {
    pub main_function: LoweredBlockPyFunction,
    pub helper_functions: Vec<LoweredBlockPyFunction>,
}

pub(crate) struct PreparedBlockPyFunction {
    pub function: BlockPyFunction,
    pub entry_label: String,
    pub generator_metadata: Option<GeneratorMetadata>,
    pub try_regions: Vec<TryRegionPlan>,
}

#[derive(Debug, Clone)]
pub(crate) struct TryRegionPlan {
    pub body_region_labels: Vec<String>,
    pub body_exception_target: String,
    pub cleanup_region_labels: Vec<String>,
    pub cleanup_exception_target: Option<String>,
}

#[derive(Clone)]
pub(crate) enum StmtSequenceHeadPlan {
    Linear(Stmt),
    FunctionDef(ast::StmtFunctionDef),
    Generator {
        plan: GeneratorStmtSequencePlan,
        sync_target_cells: bool,
    },
    Raise(ast::StmtRaise),
    Delete(ast::StmtDelete),
    Return(Option<Expr>),
    If(ast::StmtIf),
    While(ast::StmtWhile),
    For(ast::StmtFor),
    Try(ast::StmtTry),
    With(ast::StmtWith),
    Break,
    Continue,
    Unsupported,
}

pub(crate) enum StmtSequenceDriveResult {
    Exhausted {
        linear: Vec<Stmt>,
    },
    Break {
        linear: Vec<Stmt>,
        index: usize,
        plan: StmtSequenceHeadPlan,
    },
}

pub(crate) fn blockpy_kind_for_lowered_runtime(
    is_async_generator_runtime: bool,
    coroutine_via_generator: bool,
    has_yield: bool,
) -> BlockPyFunctionKind {
    if is_async_generator_runtime {
        BlockPyFunctionKind::AsyncGenerator
    } else if has_yield {
        BlockPyFunctionKind::Generator
    } else if coroutine_via_generator {
        BlockPyFunctionKind::Coroutine
    } else {
        BlockPyFunctionKind::Function
    }
}

pub(crate) fn bb_kind_for_blockpy_kind(
    kind: BlockPyFunctionKind,
    closure_state: bool,
    resume_label: &str,
    target_labels: &[String],
    resume_pcs: &[(String, usize)],
) -> BbFunctionKind {
    match kind {
        BlockPyFunctionKind::Function | BlockPyFunctionKind::Coroutine => BbFunctionKind::Function,
        BlockPyFunctionKind::Generator => BbFunctionKind::Generator {
            closure_state,
            resume_label: resume_label.to_string(),
            target_labels: target_labels.to_vec(),
            resume_pcs: resume_pcs.to_vec(),
        },
        BlockPyFunctionKind::AsyncGenerator => BbFunctionKind::AsyncGenerator {
            closure_state,
            resume_label: resume_label.to_string(),
            target_labels: target_labels.to_vec(),
            resume_pcs: resume_pcs.to_vec(),
        },
    }
}

pub(crate) fn build_blockpy_function(
    bind_name: String,
    display_name: String,
    qualname: String,
    binding_target: BindingTarget,
    kind: BlockPyFunctionKind,
    params: ast::Parameters,
    entry_label: String,
    entry_liveins: Vec<String>,
    closure_layout: Option<BbClosureLayout>,
    local_cell_slots: Vec<String>,
    mut blocks: Vec<BlockPyBlock>,
) -> BlockPyFunction {
    if let Some(entry_index) = blocks
        .iter()
        .position(|block| block.label.as_str() == entry_label.as_str())
    {
        if entry_index != 0 {
            let entry_block = blocks.remove(entry_index);
            blocks.insert(0, entry_block);
        }
    }
    BlockPyFunction {
        bind_name,
        display_name,
        qualname,
        binding_target,
        kind,
        params,
        entry_liveins,
        closure_layout,
        local_cell_slots,
        blocks,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_lowered_blockpy_function(
    function: BlockPyFunction,
    is_coroutine: bool,
    bb_kind: BbFunctionKind,
    block_params: HashMap<String, Vec<String>>,
    exception_edges: HashMap<String, Option<String>>,
    closure_layout: Option<BbClosureLayout>,
    param_specs: BbExpr,
) -> LoweredBlockPyFunction {
    LoweredBlockPyFunction {
        function,
        is_coroutine,
        bb_kind,
        block_params,
        exception_edges,
        closure_layout,
        param_specs,
    }
}

fn build_semantic_blockpy_closure_layout(
    param_names: &[String],
    entry_liveins: &[String],
    capture_names: &[String],
    local_cell_slots: &[String],
    injected_exception_names: &HashSet<String>,
) -> Option<BbClosureLayout> {
    if capture_names.is_empty()
        && local_cell_slots.is_empty()
        && injected_exception_names.is_empty()
    {
        return None;
    }

    let mut state_vars = entry_liveins.to_vec();
    for slot in local_cell_slots {
        let logical_name = slot.strip_prefix("_dp_cell_").unwrap_or(slot).to_string();
        if !state_vars.iter().any(|existing| existing == &logical_name) {
            state_vars.push(logical_name);
        }
    }

    Some(build_blockpy_closure_layout(
        param_names,
        &state_vars,
        capture_names,
        injected_exception_names,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_lowered_blockpy_function_bundle(
    prepared_function: PreparedBlockPyFunction,
    display_name: String,
    has_yield: bool,
    is_coroutine: bool,
    is_async_generator_runtime: bool,
    is_closure_backed_generator_runtime: bool,
    param_names: &[String],
    extra_closure_state_names: &[String],
    capture_names: &[String],
    label_prefix: &str,
    mut cell_slots: HashSet<String>,
    module_init_mode: bool,
    main_param_specs: BbExpr,
) -> LoweredBlockPyFunctionBundle {
    let PreparedBlockPyFunction {
        function: mut blockpy_function,
        entry_label,
        generator_metadata,
        try_regions,
    } = prepared_function;
    let exception_edges = compute_blockpy_exception_edges(
        &blockpy_function.blocks,
        &try_regions,
        generator_metadata.as_ref(),
    );
    let mut extra_successors = build_try_extra_successors(&try_regions);
    let mut blocks_for_dataflow = std::mem::take(&mut blockpy_function.blocks);
    let generator_yield_sites_for_lowering = generator_metadata
        .as_ref()
        .map(|info| info.yield_sites.clone())
        .unwrap_or_default();
    let resume_pcs = if has_yield {
        generator_metadata
            .as_ref()
            .map(|info| info.resume_order.as_slice())
            .unwrap_or(&[])
            .iter()
            .enumerate()
            .map(|(idx, label)| (label.as_str().to_string(), idx + 1))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    if has_yield {
        split_generator_return_terms_to_escape_blocks(
            &mut blocks_for_dataflow,
            &generator_yield_sites_for_lowering,
            is_async_generator_runtime,
            is_closure_backed_generator_runtime,
        );
    }

    let mut state_vars = collect_state_vars(param_names, &blocks_for_dataflow, module_init_mode);
    for block in &blocks_for_dataflow {
        let Some(exc_param) = block.exc_param.as_ref() else {
            continue;
        };
        if !state_vars.iter().any(|existing| existing == exc_param) {
            state_vars.push(exc_param.clone());
        }
    }
    if is_closure_backed_generator_runtime {
        for name in extra_closure_state_names {
            if !state_vars.iter().any(|existing| existing == name) {
                state_vars.push(name.clone());
            }
        }
    }
    if is_closure_backed_generator_runtime {
        for runtime_name in ["_dp_pc", "_dp_yieldfrom"] {
            if !state_vars.iter().any(|existing| existing == runtime_name) {
                state_vars.push(runtime_name.to_string());
            }
        }
        for dispatch_name in ["_dp_self", "_dp_send_value", "_dp_resume_exc"] {
            if !state_vars.iter().any(|existing| existing == dispatch_name) {
                state_vars.push(dispatch_name.to_string());
            }
        }
        if is_async_generator_runtime
            && !state_vars
                .iter()
                .any(|existing| existing == "_dp_transport_sent")
        {
            state_vars.push("_dp_transport_sent".to_string());
        }
    }

    for (source, target) in &exception_edges {
        let Some(target) = target.as_ref() else {
            continue;
        };
        let successors = extra_successors.entry(source.clone()).or_default();
        if !successors.iter().any(|existing| existing == target) {
            successors.push(target.clone());
        }
    }
    if has_yield && !is_closure_backed_generator_runtime {
        for site in generator_metadata
            .as_ref()
            .map(|info| info.yield_sites.as_slice())
            .unwrap_or(&[])
        {
            let successors = extra_successors
                .entry(site.yield_label.as_str().to_string())
                .or_default();
            if !successors
                .iter()
                .any(|existing| existing == site.resume_label.as_str())
            {
                successors.push(site.resume_label.as_str().to_string());
            }
        }
    }

    let mut block_params =
        compute_block_params_blockpy(&blocks_for_dataflow, &state_vars, &extra_successors);
    for block in &blocks_for_dataflow {
        let Some(exc_param) = block.exc_param.as_ref() else {
            continue;
        };
        let params = block_params
            .entry(block.label.as_str().to_string())
            .or_default();
        if !params.iter().any(|existing| existing == exc_param) {
            params.push(exc_param.clone());
        }
    }
    if has_yield {
        let injected_exc_names: Vec<String> = Vec::new();
        if is_closure_backed_generator_runtime {
            for block in &blocks_for_dataflow {
                if !generator_metadata
                    .as_ref()
                    .map(|info| {
                        info.dispatch_only_labels
                            .iter()
                            .any(|label| label.as_str() == block.label.as_str())
                    })
                    .unwrap_or(false)
                {
                    continue;
                }
                let params = block_params
                    .entry(block.label.as_str().to_string())
                    .or_default();
                let mut dispatch_params = vec![
                    "_dp_self".to_string(),
                    "_dp_send_value".to_string(),
                    "_dp_resume_exc".to_string(),
                ];
                if is_async_generator_runtime {
                    dispatch_params.push("_dp_transport_sent".to_string());
                }
                for exc_name in &injected_exc_names {
                    if params.iter().any(|name| name == exc_name) {
                        dispatch_params.push(exc_name.clone());
                    }
                }
                *params = dispatch_params;
            }
        } else {
            for block in &blocks_for_dataflow {
                let params = block_params
                    .entry(block.label.as_str().to_string())
                    .or_default();
                params.retain(|name| {
                    name != "_dp_self"
                        && name != "_dp_send_value"
                        && name != "_dp_resume_exc"
                        && name != "_dp_transport_sent"
                });
                params.insert(0, "_dp_self".to_string());
                params.insert(1, "_dp_send_value".to_string());
                params.insert(2, "_dp_resume_exc".to_string());
                if is_async_generator_runtime {
                    params.insert(3, "_dp_transport_sent".to_string());
                }
                if generator_metadata
                    .as_ref()
                    .map(|info| {
                        info.dispatch_only_labels
                            .iter()
                            .any(|label| label.as_str() == block.label.as_str())
                    })
                    .unwrap_or(false)
                {
                    params.truncate(if is_async_generator_runtime { 4 } else { 3 });
                    continue;
                }
                if block.label.as_str() != entry_label.as_str() {
                    for exc_name in &injected_exc_names {
                        if !params.iter().any(|name| name == exc_name) {
                            params.push(exc_name.clone());
                        }
                    }
                }
            }
        }
        if !injected_exc_names.is_empty() {
            if let Some(entry_block) = blocks_for_dataflow.iter_mut().find(|block| {
                block.label.as_str()
                    == generator_metadata
                        .as_ref()
                        .and_then(|info| info.dispatch_entry_label.as_deref())
                        .unwrap_or(entry_label.as_str())
            }) {
                for exc_name in injected_exc_names.iter().rev() {
                    let mut injected = lower_stmts_to_blockpy_stmts(&[py_stmt!(
                        "{name:id} = __dp_DELETED",
                        name = exc_name.as_str(),
                    )])
                    .unwrap_or_else(|err| {
                        panic!("failed to convert injected exception init to BlockPy: {err}")
                    });
                    let stmt = injected
                        .pop()
                        .expect("generated deleted-sentinel init should yield one BlockPy stmt");
                    entry_block.body.insert(0, stmt);
                }
            }
        }
    }

    let state_entry_label = generator_metadata
        .as_ref()
        .and_then(|info| info.dispatch_entry_label.as_deref())
        .unwrap_or(entry_label.as_str())
        .to_string();
    if is_closure_backed_generator_runtime {
        rewrite_sync_generator_blockpy_blocks_to_closure_cells(
            &mut blocks_for_dataflow,
            &mut block_params,
            &state_vars,
            &mut cell_slots,
            state_entry_label.as_str(),
        );
    }

    let injected_exception_names = collect_injected_exception_names_blockpy(&blocks_for_dataflow);
    let closure_layout = if is_closure_backed_generator_runtime {
        Some(build_blockpy_closure_layout(
            param_names,
            &state_vars,
            capture_names,
            &injected_exception_names,
        ))
    } else {
        None
    };
    if let (Some(uncaught_label), Some(uncaught_exc_name)) = (
        generator_metadata
            .as_ref()
            .and_then(|info| info.uncaught_block_label.as_deref()),
        generator_metadata
            .as_ref()
            .and_then(|info| info.uncaught_exc_name.as_ref()),
    ) {
        let params = block_params.entry(uncaught_label.to_string()).or_default();
        params.retain(|name| name != uncaught_exc_name);
        params.push(uncaught_exc_name.clone());
        if let Some(uncaught_set_done_label) = generator_metadata
            .as_ref()
            .and_then(|info| info.uncaught_set_done_label.as_deref())
        {
            let params = block_params
                .entry(uncaught_set_done_label.to_string())
                .or_default();
            params.retain(|name| name != uncaught_exc_name);
            params.push(uncaught_exc_name.clone());
        }
        if let Some(uncaught_raise_label) = generator_metadata
            .as_ref()
            .and_then(|info| info.uncaught_raise_label.as_deref())
        {
            let params = block_params
                .entry(uncaught_raise_label.to_string())
                .or_default();
            params.retain(|name| name != uncaught_exc_name);
            params.push(uncaught_exc_name.clone());
        }
    }
    let cleanup_cells = if is_closure_backed_generator_runtime {
        sync_generator_cleanup_cells(&state_vars, &injected_exception_names)
    } else {
        Vec::new()
    };

    if is_closure_backed_generator_runtime {
        if let Some(uncaught_set_done_label) = generator_metadata
            .as_ref()
            .and_then(|info| info.uncaught_set_done_label.as_deref())
        {
            if let Some(uncaught_set_done_block) = blocks_for_dataflow
                .iter_mut()
                .find(|block| block.label.as_str() == uncaught_set_done_label)
            {
                let mut new_body =
                    Vec::with_capacity(uncaught_set_done_block.body.len() + cleanup_cells.len());
                for stmt in std::mem::take(&mut uncaught_set_done_block.body) {
                    new_body.push(stmt);
                    if matches!(new_body.last(), Some(BlockPyStmt::Expr(_))) && new_body.len() == 1
                    {
                        for cell in &cleanup_cells {
                            new_body.extend(
                                lower_stmts_to_blockpy_stmts(&[py_stmt!(
                                    "__dp_store_cell({cell:id}, __dp_DELETED)",
                                    cell = cell.as_str(),
                                )])
                                .unwrap_or_else(|err| {
                                    panic!("failed to convert cleanup stmt to BlockPy: {err}")
                                }),
                            );
                        }
                    }
                }
                uncaught_set_done_block.body = new_body;
            }
        }
    }

    let entry_liveins = if is_closure_backed_generator_runtime {
        sync_generator_state_order(&state_vars, &injected_exception_names)
    } else {
        block_params
            .get(&state_entry_label)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|name| {
                name != "_dp_self" && name != "_dp_send_value" && name != "_dp_resume_exc"
            })
            .collect::<Vec<_>>()
    };
    let extra_state_vars = if is_closure_backed_generator_runtime {
        Vec::new()
    } else {
        entry_liveins
            .iter()
            .filter(|name| !param_names.iter().any(|param| param == *name))
            .cloned()
            .collect::<Vec<_>>()
    };
    let target_labels = blocks_for_dataflow
        .iter()
        .map(|block| block.label.as_str().to_string())
        .collect::<Vec<_>>();
    if has_yield {
        lower_generator_yield_terms_to_explicit_return_blockpy(
            &mut blocks_for_dataflow,
            &block_params,
            &resume_pcs,
            &generator_yield_sites_for_lowering,
            is_closure_backed_generator_runtime,
        );
    }

    let mut state_order = entry_liveins.clone();
    for name in extra_state_vars {
        if !state_order.iter().any(|existing| existing == &name) {
            state_order.push(name);
        }
    }

    let mut sorted_local_cell_slots = cell_slots.iter().cloned().collect::<Vec<_>>();
    sorted_local_cell_slots.sort();
    let semantic_closure_layout = if is_closure_backed_generator_runtime {
        closure_layout.clone()
    } else {
        build_semantic_blockpy_closure_layout(
            param_names,
            &state_order,
            capture_names,
            &sorted_local_cell_slots,
            &injected_exception_names,
        )
    };

    let mut exported_entry_label = entry_label.clone();
    let mut exported_entry_liveins = state_order.clone();
    let mut exported_blocks = blocks_for_dataflow;
    let mut exported_block_params = block_params.clone();
    let mut exported_exception_edges = exception_edges.clone();
    let mut helper_functions = Vec::new();
    if is_closure_backed_generator_runtime {
        let layout = closure_layout
            .as_ref()
            .expect("closure-backed generator lowering requires closure layout");
        let factory_label = format!("{label_prefix}_factory");
        let export_plan = build_closure_backed_generator_export_plan(
            factory_label.as_str(),
            entry_label.as_str(),
            blockpy_function.bind_name.as_str(),
            display_name.as_str(),
            blockpy_function.qualname.as_str(),
            param_names,
            layout,
            is_coroutine,
            is_async_generator_runtime,
            &target_labels,
            &resume_pcs,
        );
        let mut resume_local_cell_slots = cell_slots.iter().cloned().collect::<Vec<_>>();
        resume_local_cell_slots.sort();
        let resume_function = build_blockpy_function(
            export_plan.resume_bind_name.clone(),
            export_plan.resume_display_name.clone(),
            export_plan.resume_qualname.clone(),
            BindingTarget::Local,
            if is_async_generator_runtime {
                BlockPyFunctionKind::AsyncGenerator
            } else {
                BlockPyFunctionKind::Generator
            },
            blockpy_function.params.clone(),
            entry_label.clone(),
            export_plan.resume_entry_liveins.clone(),
            closure_layout.clone(),
            resume_local_cell_slots.clone(),
            exported_blocks.clone(),
        );
        let resume_blockpy_kind = resume_function.kind;
        helper_functions.push(build_lowered_blockpy_function(
            resume_function,
            false,
            bb_kind_for_blockpy_kind(
                resume_blockpy_kind,
                true,
                entry_label.as_str(),
                &target_labels,
                &resume_pcs,
            ),
            exported_block_params.clone(),
            exported_exception_edges.clone(),
            closure_layout.clone(),
            BbExpr::from_expr(export_plan.resume_param_specs.clone()),
        ));
        exported_blocks = vec![export_plan.factory_block];
        exported_entry_label = export_plan.factory_label;
        exported_entry_liveins = export_plan.factory_entry_liveins;
        exported_block_params =
            HashMap::from([(exported_entry_label.clone(), exported_entry_liveins.clone())]);
        exported_exception_edges = HashMap::new();
    }

    let main_blockpy_kind = if has_yield && !is_closure_backed_generator_runtime {
        if is_async_generator_runtime {
            BlockPyFunctionKind::AsyncGenerator
        } else {
            BlockPyFunctionKind::Generator
        }
    } else {
        BlockPyFunctionKind::Function
    };
    let main_function = build_blockpy_function(
        blockpy_function.bind_name.clone(),
        display_name.clone(),
        blockpy_function.qualname.clone(),
        blockpy_function.binding_target,
        main_blockpy_kind,
        blockpy_function.params.clone(),
        exported_entry_label.clone(),
        exported_entry_liveins.clone(),
        semantic_closure_layout,
        sorted_local_cell_slots,
        exported_blocks,
    );
    LoweredBlockPyFunctionBundle {
        main_function: build_lowered_blockpy_function(
            main_function,
            is_coroutine,
            bb_kind_for_blockpy_kind(
                main_blockpy_kind,
                is_closure_backed_generator_runtime,
                entry_label.as_str(),
                &target_labels,
                &resume_pcs,
            ),
            exported_block_params,
            exported_exception_edges,
            if is_closure_backed_generator_runtime {
                None
            } else {
                closure_layout
            },
            main_param_specs,
        ),
        helper_functions,
    }
}

pub(crate) fn build_finalized_blockpy_function(
    bind_name: String,
    qualname: String,
    binding_target: BindingTarget,
    kind: BlockPyFunctionKind,
    params: ast::Parameters,
    blocks: Vec<BlockPyBlock>,
    try_regions: Vec<TryRegionPlan>,
    entry_label: String,
    end_label: String,
    label_prefix: &str,
    generator_metadata: Option<GeneratorMetadata>,
    is_async_generator_runtime: bool,
    is_closure_backed_generator_runtime: bool,
    uncaught_exc_name: String,
) -> PreparedBlockPyFunction {
    let function = build_blockpy_function(
        bind_name.clone(),
        bind_name,
        qualname,
        binding_target,
        kind,
        params,
        entry_label.clone(),
        Vec::new(),
        None,
        Vec::new(),
        blocks,
    );
    finalize_blockpy_function(
        function,
        try_regions,
        entry_label,
        end_label,
        label_prefix,
        generator_metadata,
        is_async_generator_runtime,
        is_closure_backed_generator_runtime,
        uncaught_exc_name,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_function_body_to_blockpy_function<FDef, FTemp>(
    fn_name: &str,
    runtime_input_body: &[Box<Stmt>],
    bind_name: String,
    qualname: String,
    binding_target: BindingTarget,
    params: ast::Parameters,
    end_label: String,
    label_prefix: &str,
    has_yield: bool,
    coroutine_via_generator: bool,
    is_async_generator_runtime: bool,
    is_closure_backed_generator_runtime: bool,
    cell_slots: &HashSet<String>,
    next_block_id: &mut usize,
    lower_non_bb_def: &mut FDef,
    next_temp: &mut FTemp,
) -> PreparedBlockPyFunction
where
    FDef: FnMut(&ast::StmtFunctionDef) -> Vec<Stmt>,
    FTemp: FnMut(&str, &mut usize) -> String,
{
    let blockpy_kind = blockpy_kind_for_lowered_runtime(
        is_async_generator_runtime,
        coroutine_via_generator,
        has_yield,
    );
    let mut blocks = Vec::new();
    let mut try_regions = Vec::new();
    let mut generator_state = BlockPySequenceGeneratorState {
        enabled: has_yield,
        closure_state: is_closure_backed_generator_runtime,
        resume_order: Vec::new(),
        yield_sites: Vec::new(),
    };
    let entry_label = lower_stmt_sequence_with_state(
        fn_name,
        runtime_input_body,
        end_label.clone(),
        None,
        None,
        &mut blocks,
        cell_slots,
        &mut generator_state,
        &mut try_regions,
        next_block_id,
        lower_non_bb_def,
        next_temp,
    );
    let generator_metadata = has_yield.then(|| {
        build_initial_generator_metadata(
            entry_label.as_str(),
            &generator_state.resume_order,
            &generator_state.yield_sites,
        )
    });
    build_finalized_blockpy_function(
        bind_name,
        qualname,
        binding_target,
        blockpy_kind,
        params,
        blocks,
        try_regions,
        entry_label,
        end_label,
        label_prefix,
        generator_metadata,
        is_async_generator_runtime,
        is_closure_backed_generator_runtime,
        next_temp("uncaught_exc", next_block_id),
    )
}

fn build_try_extra_successors(try_regions: &[TryRegionPlan]) -> HashMap<String, Vec<String>> {
    let mut extra = HashMap::new();
    for region in try_regions {
        for label in &region.body_region_labels {
            extra
                .entry(label.clone())
                .or_insert_with(Vec::new)
                .push(region.body_exception_target.clone());
        }
        if let Some(cleanup_target) = region.cleanup_exception_target.as_ref() {
            for label in &region.cleanup_region_labels {
                extra
                    .entry(label.clone())
                    .or_insert_with(Vec::new)
                    .push(cleanup_target.clone());
            }
        }
    }
    extra
}

pub(crate) fn compute_blockpy_exception_edges(
    blocks: &[BlockPyBlock],
    try_regions: &[TryRegionPlan],
    generator_info: Option<&GeneratorMetadata>,
) -> HashMap<String, Option<String>> {
    let mut exception_edges = HashMap::new();
    for region in try_regions {
        let body_rank = region.body_region_labels.len();
        for label in &region.body_region_labels {
            update_try_edge_if_better(
                &mut exception_edges,
                label.clone(),
                body_rank,
                Some(region.body_exception_target.clone()),
            );
        }
        if let Some(cleanup_target) = region.cleanup_exception_target.as_ref() {
            let cleanup_rank = region.cleanup_region_labels.len();
            for label in &region.cleanup_region_labels {
                update_try_edge_if_better(
                    &mut exception_edges,
                    label.clone(),
                    cleanup_rank,
                    Some(cleanup_target.clone()),
                );
            }
        }
    }
    let mut exception_edges = exception_edges
        .into_iter()
        .map(|(label, (_rank, target))| (label, target))
        .collect::<HashMap<_, _>>();
    if let Some(generator_info) = generator_info {
        if let Some(uncaught_label) = generator_info.uncaught_block_label.as_deref() {
            for block in blocks {
                let label = block.label.as_str();
                if generator_info
                    .done_block_label
                    .as_ref()
                    .map(|done| done.as_str() == label)
                    .unwrap_or(false)
                    || generator_info
                        .invalid_block_label
                        .as_ref()
                        .map(|invalid| invalid.as_str() == label)
                        .unwrap_or(false)
                    || Some(label) == generator_info.uncaught_block_label.as_deref()
                    || generator_info
                        .throw_passthrough_labels
                        .iter()
                        .any(|passthrough| passthrough.as_str() == label)
                {
                    continue;
                }
                exception_edges
                    .entry(label.to_string())
                    .or_insert(Some(uncaught_label.to_string()));
            }
        }
    }
    exception_edges
}

fn update_try_edge_if_better(
    edges: &mut HashMap<String, (usize, Option<String>)>,
    label: String,
    rank: usize,
    target: Option<String>,
) {
    let should_update = match edges.get(label.as_str()) {
        Some((existing_rank, _)) => rank < *existing_rank,
        None => true,
    };
    if should_update {
        edges.insert(label, (rank, target));
    }
}

fn relabel_try_regions(try_regions: &mut [TryRegionPlan], rename: &HashMap<String, String>) {
    for region in try_regions {
        for label in &mut region.body_region_labels {
            if let Some(rewritten) = rename.get(label.as_str()) {
                *label = rewritten.clone();
            }
        }
        if let Some(rewritten) = rename.get(region.body_exception_target.as_str()) {
            region.body_exception_target = rewritten.clone();
        }
        for label in &mut region.cleanup_region_labels {
            if let Some(rewritten) = rename.get(label.as_str()) {
                *label = rewritten.clone();
            }
        }
        if let Some(target) = region.cleanup_exception_target.as_mut() {
            if let Some(rewritten) = rename.get(target.as_str()) {
                *target = rewritten.clone();
            }
        }
    }
}

fn relabel_generator_info(
    generator: &mut GeneratorMetadata,
    label_rename: &std::collections::HashMap<String, String>,
) {
    if let Some(dispatch_entry_label) = generator.dispatch_entry_label.as_mut() {
        if let Some(rewritten) = label_rename.get(dispatch_entry_label.as_str()) {
            *dispatch_entry_label = rewritten.clone();
        }
    }
    for label in &mut generator.resume_order {
        if let Some(rewritten) = label_rename.get(label.as_str()) {
            *label = rewritten.clone();
        }
    }
    for site in &mut generator.yield_sites {
        if let Some(rewritten) = label_rename.get(site.yield_label.as_str()) {
            site.yield_label = rewritten.clone();
        }
        if let Some(rewritten) = label_rename.get(site.resume_label.as_str()) {
            site.resume_label = rewritten.clone();
        }
    }
}

pub(crate) fn finalize_blockpy_function(
    mut function: BlockPyFunction,
    mut try_regions: Vec<TryRegionPlan>,
    mut entry_label: String,
    end_label: String,
    label_prefix: &str,
    mut generator_metadata: Option<GeneratorMetadata>,
    is_async_generator_runtime: bool,
    is_closure_backed_generator_runtime: bool,
    uncaught_exc_name: String,
) -> PreparedBlockPyFunction {
    let needs_end_block = entry_label == end_label
        || function
            .blocks
            .iter()
            .any(|block| block_references_label(block, end_label.as_str()));
    if needs_end_block {
        function.blocks.push(BlockPyBlock {
            label: BlockPyLabel::from(end_label),
            exc_param: None,
            body: Vec::new(),
            term: BlockPyTerm::Return(None),
        });
    }
    fold_jumps_to_trivial_none_return_blockpy(&mut function.blocks);
    fold_constant_brif_blockpy(&mut function.blocks);
    let prune_roots = generator_metadata
        .as_ref()
        .map(|info| info.resume_order.clone())
        .unwrap_or_default();
    prune_unreachable_blockpy_blocks(entry_label.as_str(), &prune_roots, &mut function.blocks);
    let (relabelled_entry_label, label_rename) =
        relabel_blockpy_blocks(label_prefix, entry_label.as_str(), &mut function.blocks);
    entry_label = relabelled_entry_label;
    relabel_try_regions(&mut try_regions, &label_rename);
    if let Some(generator) = generator_metadata.as_mut() {
        relabel_generator_info(generator, &label_rename);
        let resume_order = generator.resume_order.clone();
        let yield_sites = generator.yield_sites.clone();
        *generator = synthesize_generator_dispatch_metadata(
            &mut function.blocks,
            &mut entry_label,
            label_prefix,
            is_async_generator_runtime,
            is_closure_backed_generator_runtime,
            uncaught_exc_name,
            &resume_order,
            &yield_sites,
        );
    }
    PreparedBlockPyFunction {
        function,
        entry_label,
        generator_metadata,
        try_regions,
    }
}

#[derive(Clone)]
struct LoopContext {
    continue_label: BlockPyLabel,
    break_label: BlockPyLabel,
}

#[derive(Default)]
struct YieldLikeProbe {
    has_yield: bool,
    has_yield_from: bool,
}

impl Transformer for YieldLikeProbe {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if matches!(stmt, Stmt::FunctionDef(_) | Stmt::ClassDef(_)) {
            return;
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Yield(_) => self.has_yield = true,
            Expr::YieldFrom(_) => self.has_yield_from = true,
            _ => walk_expr(self, expr),
        }
    }
}

fn function_kind_from_def(func: &ast::StmtFunctionDef) -> BlockPyFunctionKind {
    let mut probe = YieldLikeProbe::default();
    for stmt in &func.body.body {
        let mut stmt = stmt.as_ref().clone();
        probe.visit_stmt(&mut stmt);
        if probe.has_yield || probe.has_yield_from {
            break;
        }
    }
    match (func.is_async, probe.has_yield || probe.has_yield_from) {
        (true, true) => BlockPyFunctionKind::AsyncGenerator,
        (true, false) => BlockPyFunctionKind::Coroutine,
        (false, true) => BlockPyFunctionKind::Generator,
        (false, false) => BlockPyFunctionKind::Function,
    }
}

fn stmt_kind_name(stmt: &Stmt) -> &'static str {
    match stmt {
        Stmt::FunctionDef(_) => "FunctionDef",
        Stmt::ClassDef(_) => "ClassDef",
        Stmt::Return(_) => "Return",
        Stmt::Delete(_) => "Delete",
        Stmt::TypeAlias(_) => "TypeAlias",
        Stmt::Assign(_) => "Assign",
        Stmt::AugAssign(_) => "AugAssign",
        Stmt::AnnAssign(_) => "AnnAssign",
        Stmt::For(_) => "For",
        Stmt::While(_) => "While",
        Stmt::If(_) => "If",
        Stmt::With(_) => "With",
        Stmt::Match(_) => "Match",
        Stmt::Raise(_) => "Raise",
        Stmt::Try(_) => "Try",
        Stmt::Assert(_) => "Assert",
        Stmt::Import(_) => "Import",
        Stmt::ImportFrom(_) => "ImportFrom",
        Stmt::Global(_) => "Global",
        Stmt::Nonlocal(_) => "Nonlocal",
        Stmt::Expr(_) => "Expr",
        Stmt::Pass(_) => "Pass",
        Stmt::Break(_) => "Break",
        Stmt::Continue(_) => "Continue",
        Stmt::IpyEscapeCommand(_) => "IpyEscapeCommand",
        Stmt::BodyStmt(_) => "BodyStmt",
    }
}

fn assign_delete_error(message: &str, stmt: &Stmt) -> String {
    format!("{message}\nstmt:\n{}", ruff_ast_to_string(stmt).trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basic_block::ruff_to_blockpy::generator_lowering::build_closure_backed_generator_factory_block;

    fn wrapped_blockpy(source: &str) -> BlockPyModule {
        crate::transform_str_to_ruff_with_options(source, crate::Options::for_test())
            .unwrap()
            .blockpy_module
            .expect("expected BlockPy module")
    }

    fn function_by_name<'a>(blockpy: &'a BlockPyModule, bind_name: &str) -> &'a BlockPyFunction {
        blockpy
            .functions
            .iter()
            .find(|func| func.bind_name == bind_name)
            .unwrap_or_else(|| panic!("missing BlockPy function {bind_name}; got {blockpy:?}"))
    }

    fn lower_stmt_for_panic_test(stmt: &Stmt) {
        let mut out = Vec::new();
        let mut next_label_id = 0usize;
        let _ = lower_stmt_into(stmt, &mut out, None, &mut next_label_id);
    }

    #[test]
    fn lowers_post_simplification_control_flow() {
        let blockpy = wrapped_blockpy(
            r#"
def f(x, ys):
    while x:
        for y in ys:
            if y:
                break
            continue
    try:
        return x
    except ValueError as err:
        return err
"#,
        );
        let blocks = &function_by_name(&blockpy, "f").blocks;
        let rendered = crate::basic_block::block_py::pretty::blockpy_module_to_string(&blockpy);
        assert!(blocks
            .iter()
            .any(|block| matches!(block.term, BlockPyTerm::IfTerm(_))));
        assert!(rendered.contains("try_jump"), "{rendered}");
        assert!(rendered.contains("return x"), "{rendered}");
    }

    #[test]
    fn lowers_async_for_structurally() {
        let blockpy = wrapped_blockpy(
            r#"
async def f(xs):
    async for x in xs:
        body(x)
"#,
        );
        let rendered = crate::basic_block::block_py::pretty::blockpy_module_to_string(&blockpy);
        assert!(rendered.contains("__dp_await_iter"), "{rendered}");
        assert!(rendered.contains("__dp_anext_or_sentinel"), "{rendered}");
    }

    #[test]
    fn lowers_generator_yield_to_explicit_blockpy_dispatch() {
        let blockpy = wrapped_blockpy(
            r#"
def gen(n):
    yield n
"#,
        );
        let rendered = crate::basic_block::blockpy_module_to_string(&blockpy);
        assert!(
            rendered.contains("function gen_resume(n)\n    kind: generator"),
            "{rendered}"
        );
        assert!(
            rendered.contains("function gen(n)\n    kind: function"),
            "{rendered}"
        );
        assert!(rendered.contains("branch_table"));
        assert!(!rendered.contains("yield n"));
    }

    #[test]
    fn shared_generator_stmt_wrapper_lowers_yield_stmt() {
        let module = ruff_python_parser::parse_module(
            r#"
def gen(n):
    yield n
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = func
            .body
            .body
            .first()
            .expect("expected generator body stmt")
            .as_ref()
            .clone();

        let mut blocks = Vec::new();
        let mut try_regions = Vec::new();
        let mut resume_order = Vec::new();
        let mut yield_sites = Vec::new();
        let mut next_block_id = 0usize;
        let plan =
            plan_generator_stmt_in_sequence(&stmt).expect("expected generator stmt sequence plan");
        assert!(plan.needs_rest_entry);
        let label = lower_generator_stmt_sequence_plan(
            &plan,
            Vec::new(),
            Some("cont".to_string()),
            &mut blocks,
            false,
            &mut try_regions,
            &mut resume_order,
            &mut yield_sites,
            &mut next_block_id,
            "gen",
            None,
        );

        assert!(label.is_some());
        assert!(!blocks.is_empty());
        assert_eq!(yield_sites.len(), 1);
    }

    #[test]
    fn generator_sequence_head_marks_yield_stmt_as_needing_rest() {
        let module = ruff_python_parser::parse_module(
            r#"
def gen(n):
    yield n
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = func
            .body
            .body
            .first()
            .expect("expected generator body stmt")
            .as_ref()
            .clone();

        let needs_rest_entry = plan_generator_stmt_in_sequence(&stmt)
            .expect("expected generator sequence head")
            .needs_rest_entry;

        assert!(needs_rest_entry);
    }

    #[test]
    fn generator_sequence_head_marks_return_yield_as_not_needing_rest() {
        let module = ruff_python_parser::parse_module(
            r#"
def gen(n):
    return (yield n)
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = func
            .body
            .body
            .first()
            .expect("expected generator body stmt")
            .as_ref()
            .clone();

        let needs_rest_entry = plan_generator_stmt_in_sequence(&stmt)
            .expect("expected generator sequence head")
            .needs_rest_entry;

        assert!(!needs_rest_entry);
    }

    #[test]
    fn stmt_sequence_head_plan_marks_yield_expr_as_generator() {
        let module = ruff_python_parser::parse_module(
            r#"
def gen():
    yield x
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = func.body.body[0].as_ref();

        assert!(matches!(
            plan_stmt_sequence_head(stmt, true),
            StmtSequenceHeadPlan::Generator {
                sync_target_cells: false,
                ..
            }
        ));
    }

    #[test]
    fn stmt_sequence_head_plan_marks_assign_yield_as_generator_with_cell_sync() {
        let module = ruff_python_parser::parse_module(
            r#"
def gen():
    x = (yield y)
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = func.body.body[0].as_ref();

        assert!(matches!(
            plan_stmt_sequence_head(stmt, true),
            StmtSequenceHeadPlan::Generator {
                sync_target_cells: true,
                ..
            }
        ));
    }

    #[test]
    fn stmt_sequence_head_plan_keeps_plain_return_as_plain_return() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    return x
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = func.body.body[0].as_ref();

        assert!(matches!(
            plan_stmt_sequence_head(stmt, false),
            StmtSequenceHeadPlan::Return(_)
        ));
        assert!(matches!(
            plan_stmt_sequence_head(stmt, true),
            StmtSequenceHeadPlan::Return(_)
        ));
    }

    #[test]
    fn generator_stmt_sequence_head_skips_plain_return_value() {
        let module = ruff_python_parser::parse_module(
            r#"
async def run():
    return 1
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        assert!(func.is_async);
        let stmt = func.body.body[0].as_ref();

        assert!(plan_generator_stmt_in_sequence(stmt).is_none());
    }

    #[test]
    fn prepare_generator_stmt_plan_from_stmt_only_lowers_rest_when_needed() {
        let module = ruff_python_parser::parse_module(
            r#"
def gen(n):
    yield n
    after()
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = func.body.body[0].as_ref();
        let remaining = vec![func.body.body[1].clone()];

        let mut blocks = Vec::new();
        let mut try_regions = Vec::new();
        let mut rest_calls = Vec::new();

        let generator_plan =
            plan_generator_stmt_in_sequence(stmt).expect("expected generator stmt sequence plan");
        let (label, state) = lower_generator_stmt_sequence_head(
            "gen",
            generator_plan,
            &remaining,
            "cont".to_string(),
            Vec::new(),
            &mut blocks,
            GeneratorStmtSequenceLoweringState {
                enabled: true,
                closure_state: false,
                resume_order: Vec::new(),
                yield_sites: Vec::new(),
                next_block_id: 0,
            },
            &mut try_regions,
            None,
            &mut |stmts, cont_label, _blocks, state| {
                rest_calls.push((stmts.len(), cont_label.clone()));
                ("rest_entry".to_string(), state)
            },
        );

        assert_eq!(rest_calls, vec![(1, "cont".to_string())]);
        assert!(label.is_some());
        assert_eq!(state.yield_sites.len(), 1);
    }

    #[test]
    fn prepare_generator_stmt_plan_from_stmt_skips_rest_when_not_needed() {
        let module = ruff_python_parser::parse_module(
            r#"
def gen(n):
    return (yield n)
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = func.body.body[0].as_ref();

        let mut blocks = Vec::new();
        let mut try_regions = Vec::new();
        let mut rest_called = false;

        let generator_plan =
            plan_generator_stmt_in_sequence(stmt).expect("expected generator stmt sequence plan");
        let (label, state) = lower_generator_stmt_sequence_head(
            "gen",
            generator_plan,
            &[],
            "cont".to_string(),
            Vec::new(),
            &mut blocks,
            GeneratorStmtSequenceLoweringState {
                enabled: true,
                closure_state: false,
                resume_order: Vec::new(),
                yield_sites: Vec::new(),
                next_block_id: 0,
            },
            &mut try_regions,
            None,
            &mut |_stmts, _cont_label, _blocks, state| {
                rest_called = true;
                ("rest_entry".to_string(), state)
            },
        );

        assert!(!rest_called);
        assert!(label.is_some());
        assert_eq!(state.yield_sites.len(), 1);
    }

    #[test]
    fn lower_for_loop_continue_entry_with_state_returns_loop_check_for_sync_for() {
        let mut blocks = Vec::new();
        let mut try_regions = Vec::new();
        let (entry, state) = lower_for_loop_continue_entry_with_state(
            &mut blocks,
            "demo",
            "_dp_iter_0",
            "_dp_tmp_0",
            "_dp_bb_demo_0".to_string(),
            false,
            &mut try_regions,
            GeneratorStmtSequenceLoweringState {
                enabled: true,
                closure_state: false,
                resume_order: Vec::new(),
                yield_sites: Vec::new(),
                next_block_id: 0,
            },
        );

        assert_eq!(entry, "_dp_bb_demo_0");
        assert!(blocks.is_empty());
        assert!(state.resume_order.is_empty());
        assert!(state.yield_sites.is_empty());
        assert_eq!(state.next_block_id, 0);
    }

    #[test]
    fn lower_for_loop_continue_entry_with_state_updates_async_generator_state() {
        let mut blocks = Vec::new();
        let mut try_regions = Vec::new();
        let (entry, state) = lower_for_loop_continue_entry_with_state(
            &mut blocks,
            "demo",
            "_dp_iter_0",
            "_dp_tmp_0",
            "_dp_bb_demo_0".to_string(),
            true,
            &mut try_regions,
            GeneratorStmtSequenceLoweringState {
                enabled: true,
                closure_state: false,
                resume_order: Vec::new(),
                yield_sites: Vec::new(),
                next_block_id: 0,
            },
        );

        assert!(!blocks.is_empty());
        assert_ne!(entry, "_dp_bb_demo_0");
        assert!(!state.resume_order.is_empty());
        assert_eq!(state.yield_sites.len(), 1);
        assert!(state.next_block_id > 0);
    }

    #[test]
    fn lower_for_stmt_sequence_emits_loop_scaffolding() {
        let module = ruff_python_parser::parse_module(
            r#"
def f(xs):
    for x in xs:
        body(x)
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let ast::Stmt::For(for_stmt) = func.body.body[0].as_ref() else {
            panic!("expected for stmt");
        };

        let mut blocks = Vec::new();
        let entry = lower_for_stmt_sequence(
            for_stmt.clone(),
            &[],
            "cont".to_string(),
            Vec::new(),
            &mut blocks,
            "_dp_iter_0",
            "_dp_tmp_0",
            "_dp_bb_demo_0".to_string(),
            "_dp_bb_demo_0".to_string(),
            "_dp_bb_demo_1".to_string(),
            "_dp_bb_demo_2".to_string(),
            vec![py_stmt!("x = _dp_tmp_0"), py_stmt!("_dp_tmp_0 = None")],
            &mut |_stmts, cont_label, _break_label, _blocks| cont_label,
        );

        assert_eq!(entry, "_dp_bb_demo_2");
        assert!(blocks
            .iter()
            .any(|block| block.label.as_str() == "_dp_bb_demo_1"));
        assert!(blocks
            .iter()
            .any(|block| block.label.as_str() == "_dp_bb_demo_2"));
    }

    #[test]
    fn builds_closure_backed_generator_factory_block() {
        let layout = crate::basic_block::bb_ir::BbClosureLayout {
            freevars: vec![crate::basic_block::bb_ir::BbClosureSlot {
                logical_name: "captured".to_string(),
                storage_name: "_dp_cell_captured".to_string(),
                init: crate::basic_block::bb_ir::BbClosureInit::InheritedCapture,
            }],
            cellvars: vec![crate::basic_block::bb_ir::BbClosureSlot {
                logical_name: "x".to_string(),
                storage_name: "_dp_cell_x".to_string(),
                init: crate::basic_block::bb_ir::BbClosureInit::Parameter,
            }],
            runtime_cells: vec![crate::basic_block::bb_ir::BbClosureSlot {
                logical_name: "_dp_pc".to_string(),
                storage_name: "_dp_cell__dp_pc".to_string(),
                init: crate::basic_block::bb_ir::BbClosureInit::RuntimePcUnstarted,
            }],
        };

        let block = build_closure_backed_generator_factory_block(
            "_dp_bb_demo_factory",
            "_dp_bb_demo_0",
            &[
                "_dp_self".to_string(),
                "_dp_send_value".to_string(),
                "_dp_resume_exc".to_string(),
                "_dp_cell_captured".to_string(),
                "_dp_cell_x".to_string(),
                "_dp_cell__dp_pc".to_string(),
            ],
            "demo",
            "demo",
            &layout,
            false,
            false,
        );

        assert_eq!(block.label.as_str(), "_dp_bb_demo_factory");
        assert!(matches!(block.term, BlockPyTerm::Return(Some(_))));
    }

    #[test]
    fn lower_with_stmt_sequence_emits_shared_cleanup_try_jump() {
        let module = ruff_python_parser::parse_module(
            r#"
def f(ctx, value):
    with ctx() as value:
        body()
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let ast::Stmt::With(with_stmt) = func.body.body[0].as_ref() else {
            panic!("expected with stmt");
        };

        let mut blocks = Vec::new();
        let next_block_id = Cell::new(0usize);
        let (entry, try_region) = lower_with_stmt_sequence(
            "demo",
            with_stmt.clone(),
            &[],
            "cont".to_string(),
            Vec::new(),
            &mut blocks,
            &HashSet::new(),
            &next_block_id,
            false,
            &mut |_expanded, cont_label, _blocks| {
                assert!(cont_label == "cont" || cont_label.ends_with("__normal"));
                cont_label
            },
        );

        assert!(!entry.is_empty());
        assert!(try_region.is_some());
        assert!(blocks
            .iter()
            .any(|block| matches!(block.term, BlockPyTerm::TryJump(_))));
        assert!(!blocks
            .iter()
            .flat_map(|block| block.body.iter())
            .any(|stmt| {
                matches!(stmt,
                    BlockPyStmt::Assign(assign)
                        if assign.target.id.as_str().contains("with_ok")
                            || assign.target.id.as_str().contains("with_suppress")
                )
            }));
    }

    #[test]
    fn lower_try_stmt_sequence_emits_try_jump() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    try:
        body()
    except ValueError:
        handle()
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let ast::Stmt::Try(try_stmt) = func.body.body[0].as_ref() else {
            panic!("expected try stmt");
        };

        let mut blocks = Vec::new();
        let mut next_label_id = 0usize;
        let try_plan = build_try_plan("demo", false, false, &mut next_label_id);
        let (entry, try_region) = lower_try_stmt_sequence(
            try_stmt.clone(),
            &[],
            "cont".to_string(),
            Vec::new(),
            &mut blocks,
            "_dp_bb_demo_legacy".to_string(),
            try_plan,
            &mut |_expanded, cont_label, _blocks| cont_label,
        );

        assert!(!entry.is_empty());
        assert_eq!(try_region.body_exception_target, "cont");
        assert!(blocks
            .iter()
            .any(|block| matches!(block.term, BlockPyTerm::TryJump(_))));
    }

    #[test]
    fn expanded_stmt_helper_returns_expanded_entry_without_linear_prefix() {
        let mut blocks = Vec::new();
        let mut saw_expanded = false;
        let entry = lower_expanded_stmt_sequence(
            py_stmt!("pass"),
            &[],
            "cont".to_string(),
            Vec::new(),
            &mut blocks,
            None,
            &mut |expanded, cont_label, _blocks| {
                assert_eq!(expanded.len(), 1);
                assert_eq!(cont_label, "cont");
                saw_expanded = true;
                "expanded_entry".to_string()
            },
        );

        assert!(saw_expanded);
        assert_eq!(entry, "expanded_entry");
        assert!(blocks.is_empty());
    }

    #[test]
    fn expanded_stmt_helper_emits_linear_jump_prefix() {
        let mut blocks = Vec::new();
        let entry = lower_expanded_stmt_sequence(
            py_stmt!("pass"),
            &[],
            "cont".to_string(),
            vec![py_stmt!("x = 1")],
            &mut blocks,
            Some("prefix".to_string()),
            &mut |_expanded, _cont_label, _blocks| "expanded_entry".to_string(),
        );

        assert_eq!(entry, "prefix");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].label.as_str(), "prefix");
        assert!(matches!(
            &blocks[0].term,
            BlockPyTerm::Jump(label) if label.as_str() == "expanded_entry"
        ));
    }

    #[test]
    fn if_stmt_helper_lowers_both_branches_via_callback() {
        let mut blocks = Vec::new();
        let then_body = vec![Box::new(py_stmt!("x = 1"))];
        let else_body = vec![Box::new(py_stmt!("x = 2"))];
        let mut calls = Vec::new();

        let entry = lower_if_stmt_sequence(
            &mut blocks,
            "if_label".to_string(),
            vec![py_stmt!("prefix = 0")],
            py_expr!("flag"),
            &then_body,
            &else_body,
            "rest".to_string(),
            &mut |stmts, cont_label, _blocks| {
                calls.push((stmts.len(), cont_label.clone()));
                format!("branch_{}", calls.len())
            },
        );

        assert_eq!(entry, "if_label");
        assert_eq!(
            calls,
            vec![
                (then_body.len(), "rest".to_string()),
                (else_body.len(), "rest".to_string())
            ]
        );
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0].term, BlockPyTerm::IfTerm(_)));
    }

    #[test]
    fn sequence_jump_helper_emits_jump_block() {
        let mut blocks = Vec::new();
        let entry = emit_sequence_jump_block(
            &mut blocks,
            "jump_label".to_string(),
            vec![py_stmt!("prefix = 0")],
            "target".to_string(),
        );

        assert_eq!(entry, "jump_label");
        assert_eq!(blocks.len(), 1);
        assert!(matches!(
            &blocks[0].term,
            BlockPyTerm::Jump(label) if label.as_str() == "target"
        ));
    }

    #[test]
    fn sequence_return_helper_emits_return_block() {
        let mut blocks = Vec::new();
        let entry = emit_sequence_return_block(
            &mut blocks,
            "ret_label".to_string(),
            vec![py_stmt!("prefix = 0")],
            Some(py_expr!("value")),
        );

        assert_eq!(entry, "ret_label");
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0].term, BlockPyTerm::Return(Some(_))));
    }

    #[test]
    fn sequence_raise_helper_emits_raise_block() {
        let mut blocks = Vec::new();
        let entry = emit_sequence_raise_block(
            &mut blocks,
            "raise_label".to_string(),
            vec![py_stmt!("prefix = 0")],
            BlockPyRaise {
                exc: Some(py_expr!("exc").into()),
            },
        );

        assert_eq!(entry, "raise_label");
        assert_eq!(blocks.len(), 1);
        assert!(matches!(
            blocks[0].term,
            BlockPyTerm::Raise(BlockPyRaise { exc: Some(_) })
        ));
    }

    #[test]
    fn if_stmt_from_stmt_helper_lowers_remaining_and_branches() {
        let module = ruff_python_parser::parse_module(
            r#"
if flag:
    x = 1
else:
    x = 2
y = 3
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::If(if_stmt) = module.body[0].as_ref() else {
            panic!("expected if stmt");
        };
        let remaining = vec![module.body[1].clone()];
        let mut blocks = Vec::new();
        let mut calls = Vec::new();

        let entry = lower_if_stmt_sequence_from_stmt(
            if_stmt.clone(),
            &remaining,
            "cont".to_string(),
            vec![py_stmt!("prefix = 0")],
            &mut blocks,
            "if_label".to_string(),
            &mut |stmts, cont_label, _blocks| {
                calls.push((stmts.len(), cont_label.clone()));
                format!("branch_{}", calls.len())
            },
        );

        assert_eq!(entry, "if_label");
        assert_eq!(
            calls,
            vec![
                (remaining.len(), "cont".to_string()),
                (1, "branch_1".to_string()),
                (1, "branch_1".to_string())
            ]
        );
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0].term, BlockPyTerm::IfTerm(_)));
    }

    #[test]
    fn while_stmt_helper_lowers_loop_and_else_via_callbacks() {
        let mut blocks = Vec::new();
        let body = vec![Box::new(py_stmt!("x = 1"))];
        let else_body = vec![Box::new(py_stmt!("x = 2"))];
        let remaining = vec![Box::new(py_stmt!("x = 3"))];
        let mut sequence_calls = Vec::new();
        let mut loop_calls = Vec::new();

        let entry = lower_while_stmt_sequence(
            &mut blocks,
            "_dp_bb_loop_fn_0".to_string(),
            Some("_dp_bb_loop_fn_1".to_string()),
            vec![py_stmt!("prefix = 0")],
            py_expr!("flag"),
            &body,
            &else_body,
            &remaining,
            "cont".to_string(),
            &mut |stmts, cont_label, break_label, _blocks| {
                if let Some(break_label) = break_label {
                    loop_calls.push((stmts.len(), cont_label.clone(), break_label));
                    "loop_body".to_string()
                } else {
                    sequence_calls.push((stmts.len(), cont_label.clone()));
                    format!("seq_{}", sequence_calls.len())
                }
            },
        );

        assert_eq!(entry, "_dp_bb_loop_fn_1");
        assert_eq!(
            sequence_calls,
            vec![
                (remaining.len(), "cont".to_string()),
                (else_body.len(), "seq_1".to_string())
            ]
        );
        assert_eq!(
            loop_calls,
            vec![(
                body.len(),
                "_dp_bb_loop_fn_0".to_string(),
                "seq_1".to_string()
            )]
        );
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].label.as_str(), "_dp_bb_loop_fn_0");
        assert_eq!(blocks[1].label.as_str(), "_dp_bb_loop_fn_1");
    }

    #[test]
    fn while_stmt_from_stmt_helper_lowers_remaining_loop_and_else() {
        let module = ruff_python_parser::parse_module(
            r#"
while flag:
    x = 1
else:
    x = 2
y = 3
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::While(while_stmt) = module.body[0].as_ref() else {
            panic!("expected while stmt");
        };
        let remaining = vec![module.body[1].clone()];
        let mut blocks = Vec::new();
        let mut sequence_calls = Vec::new();
        let mut loop_calls = Vec::new();

        let entry = lower_while_stmt_sequence_from_stmt(
            while_stmt.clone(),
            &remaining,
            "cont".to_string(),
            vec![py_stmt!("prefix = 0")],
            &mut blocks,
            "_dp_bb_loop_fn_0".to_string(),
            Some("_dp_bb_loop_fn_1".to_string()),
            &mut |stmts, cont_label, break_label, _blocks| {
                if let Some(break_label) = break_label {
                    loop_calls.push((stmts.len(), cont_label.clone(), break_label));
                    "loop_body".to_string()
                } else {
                    sequence_calls.push((stmts.len(), cont_label.clone()));
                    format!("seq_{}", sequence_calls.len())
                }
            },
        );

        assert_eq!(entry, "_dp_bb_loop_fn_1");
        assert_eq!(
            sequence_calls,
            vec![
                (remaining.len(), "cont".to_string()),
                (1, "seq_1".to_string())
            ]
        );
        assert_eq!(
            loop_calls,
            vec![(1, "_dp_bb_loop_fn_0".to_string(), "seq_1".to_string())]
        );
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].label.as_str(), "_dp_bb_loop_fn_0");
        assert_eq!(blocks[1].label.as_str(), "_dp_bb_loop_fn_1");
    }

    #[test]
    fn lowers_generator_yield_from_to_explicit_blockpy_dispatch() {
        let blockpy = wrapped_blockpy(
            r#"
def gen(it):
    yield from it
"#,
        );
        let rendered = crate::basic_block::blockpy_module_to_string(&blockpy);
        assert!(rendered.contains("branch_table"));
        assert!(!rendered.contains("yield from it"));
    }

    #[test]
    fn lowers_async_generator_yield_to_explicit_blockpy_dispatch() {
        let blockpy = wrapped_blockpy(
            r#"
async def agen(n):
    yield n
"#,
        );
        let rendered = crate::basic_block::blockpy_module_to_string(&blockpy);
        assert!(rendered.contains("branch_table"));
        assert!(!rendered.contains("yield n"));
    }

    #[test]
    #[should_panic(expected = "Assert should be lowered before Ruff AST -> BlockPy conversion")]
    fn panics_if_assert_reaches_blockpy() {
        let module = ruff_python_parser::parse_module(
            r#"
def f(x):
    assert x
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        lower_stmt_for_panic_test(func.body.body[0].as_ref());
    }

    #[test]
    #[should_panic(expected = "ClassDef should be lowered before Ruff AST -> BlockPy conversion")]
    fn panics_if_classdef_reaches_blockpy() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    class X:
        pass
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        lower_stmt_for_panic_test(func.body.body[0].as_ref());
    }

    #[test]
    #[should_panic(expected = "AugAssign should be lowered before Ruff AST -> BlockPy conversion")]
    fn panics_if_augassign_reaches_blockpy() {
        let module = ruff_python_parser::parse_module(
            r#"
def f(x):
    x += 1
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        lower_stmt_for_panic_test(func.body.body[0].as_ref());
    }

    #[test]
    #[should_panic(expected = "AnnAssign should be lowered before Ruff AST -> BlockPy conversion")]
    fn panics_if_annassign_reaches_blockpy() {
        let module = ruff_python_parser::parse_module(
            r#"
def f(x):
    y: int = x
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        lower_stmt_for_panic_test(func.body.body[0].as_ref());
    }

    #[test]
    #[should_panic(expected = "TypeAlias should be lowered before Ruff AST -> BlockPy conversion")]
    fn panics_if_typealias_reaches_blockpy() {
        let module = ruff_python_parser::parse_module(
            r#"
type X = int

def f():
    return 1
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        lower_stmt_for_panic_test(module.body[0].as_ref());
    }

    #[test]
    #[should_panic(expected = "Match should be lowered before Ruff AST -> BlockPy conversion")]
    fn panics_if_match_reaches_blockpy() {
        let module = ruff_python_parser::parse_module(
            r#"
def f(x):
    match x:
        case 1:
            return 1
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        lower_stmt_for_panic_test(func.body.body[0].as_ref());
    }

    #[test]
    #[should_panic(expected = "Import should be lowered before Ruff AST -> BlockPy conversion")]
    fn panics_if_import_reaches_blockpy() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    import os
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        lower_stmt_for_panic_test(func.body.body[0].as_ref());
    }

    #[test]
    #[should_panic(expected = "ImportFrom should be lowered before Ruff AST -> BlockPy conversion")]
    fn panics_if_importfrom_reaches_blockpy() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    from math import sqrt
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        lower_stmt_for_panic_test(func.body.body[0].as_ref());
    }

    #[test]
    fn lowers_bare_raise_to_optional_blockpy_raise() {
        let blockpy = wrapped_blockpy(
            r#"
def f():
    raise
"#,
        );
        let raise_stmt = match &function_by_name(&blockpy, "f").blocks[0].term {
            BlockPyTerm::Raise(raise_stmt) => raise_stmt,
            other => panic!("expected BlockPy raise term, got {other:?}"),
        };
        assert!(raise_stmt.exc.is_none());
    }

    #[test]
    #[should_panic(expected = "raise-from should be lowered before Ruff AST -> BlockPy conversion")]
    fn panics_if_raise_from_reaches_blockpy() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    raise E from cause
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        lower_stmt_for_panic_test(func.body.body[0].as_ref());
    }

    #[test]
    #[should_panic(
        expected = "While should be lowered before Ruff AST -> BlockPy stmt-list conversion"
    )]
    fn panics_if_while_reaches_stmt_list_lowering() {
        let module = ruff_python_parser::parse_module("while x:\n    pass\n")
            .unwrap()
            .into_syntax()
            .body;
        let ast::Stmt::While(while_stmt) = module.body[0].as_ref() else {
            panic!("expected while stmt");
        };
        let mut out = Vec::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(
            &Stmt::While(while_stmt.clone()),
            &mut out,
            None,
            &mut next_label_id,
        )
        .unwrap();
    }
}
