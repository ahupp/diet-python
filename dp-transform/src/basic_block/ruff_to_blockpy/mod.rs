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
use super::deleted_names::rewrite_delete_to_deleted_sentinel;
use super::function_identity::FunctionIdentityByNode;
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

mod generator_lowering;

pub(crate) use generator_lowering::{
    blockpy_stmt_requires_generator_rest_entry, build_async_for_continue_entry,
    build_closure_backed_generator_export_plan, build_initial_generator_metadata,
    build_resume_closure_layout, lower_generator_blockpy_stmt_in_sequence,
    lower_generator_yield_terms_to_explicit_return_blockpy,
    split_generator_return_terms_to_escape_blocks, synthesize_generator_dispatch_metadata,
    GeneratorMetadata, GeneratorYieldSite,
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
    pub display_name: String,
    pub is_coroutine: bool,
    pub bb_kind: BbFunctionKind,
    pub entry_label: String,
    pub entry_params: Vec<String>,
    pub block_params: HashMap<String, Vec<String>>,
    pub exception_edges: HashMap<String, Option<String>>,
    pub closure_layout: Option<BbClosureLayout>,
    pub param_specs: BbExpr,
    pub local_cell_slots: HashSet<String>,
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
    qualname: String,
    binding_target: BindingTarget,
    kind: BlockPyFunctionKind,
    params: ast::Parameters,
    blocks: Vec<BlockPyBlock>,
) -> BlockPyFunction {
    BlockPyFunction {
        bind_name,
        qualname,
        binding_target,
        kind,
        params,
        blocks,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_lowered_blockpy_function(
    function: BlockPyFunction,
    display_name: String,
    is_coroutine: bool,
    bb_kind: BbFunctionKind,
    entry_label: String,
    entry_params: Vec<String>,
    block_params: HashMap<String, Vec<String>>,
    exception_edges: HashMap<String, Option<String>>,
    closure_layout: Option<BbClosureLayout>,
    param_specs: BbExpr,
    local_cell_slots: HashSet<String>,
) -> LoweredBlockPyFunction {
    LoweredBlockPyFunction {
        function,
        display_name,
        is_coroutine,
        bb_kind,
        entry_label,
        entry_params,
        block_params,
        exception_edges,
        closure_layout,
        param_specs,
        local_cell_slots,
    }
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
    generator_capture_names: &[String],
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
        Some(build_resume_closure_layout(
            param_names,
            &state_vars,
            generator_capture_names,
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

    let entry_params = if is_closure_backed_generator_runtime {
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
        entry_params
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

    let mut state_order = entry_params.clone();
    for name in extra_state_vars {
        if !state_order.iter().any(|existing| existing == &name) {
            state_order.push(name);
        }
    }

    let mut exported_entry_label = entry_label.clone();
    let mut exported_entry_params = state_order.clone();
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
            export_plan.resume_qualname.clone(),
            BindingTarget::Local,
            if is_async_generator_runtime {
                BlockPyFunctionKind::AsyncGenerator
            } else {
                BlockPyFunctionKind::Generator
            },
            blockpy_function.params.clone(),
            exported_blocks.clone(),
        );
        let resume_blockpy_kind = resume_function.kind;
        helper_functions.push(build_lowered_blockpy_function(
            resume_function,
            export_plan.resume_display_name,
            false,
            bb_kind_for_blockpy_kind(
                resume_blockpy_kind,
                true,
                entry_label.as_str(),
                &target_labels,
                &resume_pcs,
            ),
            entry_label.clone(),
            export_plan.resume_entry_params,
            exported_block_params.clone(),
            exported_exception_edges.clone(),
            closure_layout.clone(),
            BbExpr::from_expr(export_plan.resume_param_specs.clone()),
            resume_local_cell_slots.into_iter().collect(),
        ));
        exported_blocks = vec![export_plan.factory_block];
        exported_entry_label = export_plan.factory_label;
        exported_entry_params = export_plan.factory_entry_params;
        exported_block_params =
            HashMap::from([(exported_entry_label.clone(), exported_entry_params.clone())]);
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
        blockpy_function.qualname.clone(),
        blockpy_function.binding_target,
        main_blockpy_kind,
        blockpy_function.params.clone(),
        exported_blocks,
    );
    LoweredBlockPyFunctionBundle {
        main_function: build_lowered_blockpy_function(
            main_function,
            display_name,
            is_coroutine,
            bb_kind_for_blockpy_kind(
                main_blockpy_kind,
                is_closure_backed_generator_runtime,
                entry_label.as_str(),
                &target_labels,
                &resume_pcs,
            ),
            exported_entry_label,
            exported_entry_params,
            exported_block_params,
            exported_exception_edges,
            if is_closure_backed_generator_runtime {
                None
            } else {
                closure_layout
            },
            main_param_specs,
            cell_slots,
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
    let function =
        build_blockpy_function(bind_name, qualname, binding_target, kind, params, blocks);
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

pub(crate) fn compat_block_from_blockpy(
    label: String,
    body: Vec<Stmt>,
    term: BlockPyTerm,
) -> BlockPyBlock {
    let body = lower_stmts_to_blockpy_stmts(&body).unwrap_or_else(|err| {
        panic!("failed to convert compatibility block body to BlockPy: {err}")
    });
    BlockPyBlock {
        label: BlockPyLabel::from(label),
        exc_param: None,
        body,
        term,
    }
}

pub(crate) fn compat_if_jump_block(
    label: String,
    body: Vec<Stmt>,
    test: Expr,
    then_label: String,
    else_label: String,
) -> BlockPyBlock {
    compat_block_from_blockpy(
        label.clone(),
        body,
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test: test.into(),
            body: Box::new(BlockPyBlock {
                label: BlockPyLabel::from(format!("{label}_then")),
                exc_param: None,
                body: Vec::new(),
                term: BlockPyTerm::Jump(BlockPyLabel::from(then_label)),
            }),
            orelse: Box::new(BlockPyBlock {
                label: BlockPyLabel::from(format!("{label}_else")),
                exc_param: None,
                body: Vec::new(),
                term: BlockPyTerm::Jump(BlockPyLabel::from(else_label)),
            }),
        }),
    )
}

pub(crate) fn compat_jump_block_from_blockpy(
    label: String,
    body: Vec<Stmt>,
    target_label: String,
) -> BlockPyBlock {
    compat_block_from_blockpy(
        label,
        body,
        BlockPyTerm::Jump(BlockPyLabel::from(target_label)),
    )
}

pub(crate) fn compat_return_block_from_expr(
    label: String,
    body: Vec<Stmt>,
    value: Option<Expr>,
) -> BlockPyBlock {
    compat_block_from_blockpy(label, body, BlockPyTerm::Return(value.map(Into::into)))
}

pub(crate) fn compat_raise_block_from_blockpy_raise(
    label: String,
    body: Vec<Stmt>,
    exc: BlockPyRaise,
) -> BlockPyBlock {
    compat_block_from_blockpy(label, body, BlockPyTerm::Raise(exc))
}

fn term_from_legacy_stmt(stmt: &BlockPyStmt) -> Option<BlockPyTerm> {
    BlockPyTerm::from_stmt(stmt)
}

pub(crate) fn finalize_blockpy_block(
    label: BlockPyLabel,
    mut body: Vec<BlockPyStmt>,
    fallthrough_target: Option<BlockPyLabel>,
) -> BlockPyBlock {
    let explicit_term = body.last().and_then(term_from_legacy_stmt);
    if explicit_term.is_some() {
        body.pop();
    }
    let term = explicit_term.unwrap_or_else(|| match fallthrough_target {
        Some(target) => BlockPyTerm::Jump(target),
        None => BlockPyTerm::Return(None),
    });
    BlockPyBlock {
        label,
        exc_param: None,
        body,
        term,
    }
}

fn set_block_exc_param(blocks: &mut [BlockPyBlock], label: &str, exc_param: &str) {
    let block = blocks
        .iter_mut()
        .find(|block| block.label.as_str() == label)
        .unwrap_or_else(|| panic!("missing BlockPy block {label} for exception param"));
    if block.exc_param.is_none() {
        block.exc_param = Some(exc_param.to_string());
    }
}

fn set_region_exc_param(
    blocks: &mut [BlockPyBlock],
    region: &std::ops::Range<usize>,
    exc_param: &str,
) {
    for block in &mut blocks[region.clone()] {
        if block.exc_param.is_none() {
            block.exc_param = Some(exc_param.to_string());
        }
    }
}

pub(crate) fn emit_sequence_jump_block(
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    linear: Vec<Stmt>,
    target_label: String,
) -> String {
    blocks.push(compat_jump_block_from_blockpy(
        label.clone(),
        linear,
        target_label,
    ));
    label
}

pub(crate) fn emit_sequence_return_block(
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    linear: Vec<Stmt>,
    value: Option<Expr>,
) -> String {
    blocks.push(compat_return_block_from_expr(label.clone(), linear, value));
    label
}

pub(crate) fn emit_sequence_raise_block(
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    linear: Vec<Stmt>,
    exc: BlockPyRaise,
) -> String {
    blocks.push(compat_raise_block_from_blockpy_raise(
        label.clone(),
        linear,
        exc,
    ));
    label
}

pub(crate) fn emit_if_branch_block(
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    body: Vec<Stmt>,
    test: Expr,
    then_label: String,
    else_label: String,
) -> String {
    blocks.push(compat_if_jump_block(
        label.clone(),
        body,
        test,
        then_label,
        else_label,
    ));
    label
}

pub(crate) fn emit_simple_while_blocks(
    blocks: &mut Vec<BlockPyBlock>,
    test_label: String,
    linear_label: Option<String>,
    linear: Vec<Stmt>,
    test: Expr,
    body_entry: String,
    cond_false_entry: String,
) -> String {
    blocks.push(compat_if_jump_block(
        test_label.clone(),
        Vec::new(),
        test,
        body_entry,
        cond_false_entry,
    ));
    if let Some(linear_label) = linear_label {
        blocks.push(compat_jump_block_from_blockpy(
            linear_label.clone(),
            linear,
            test_label,
        ));
        linear_label
    } else {
        test_label
    }
}

pub(crate) fn emit_for_loop_blocks(
    blocks: &mut Vec<BlockPyBlock>,
    setup_label: String,
    assign_label: String,
    loop_check_label: String,
    loop_continue_label: String,
    linear: Vec<Stmt>,
    iter_name: &str,
    tmp_name: &str,
    iterable: Expr,
    is_async: bool,
    exhausted_entry: String,
    body_entry: String,
    assign_body: Vec<Stmt>,
) -> String {
    let iter_expr = py_expr!("{iter:id}", iter = iter_name);
    let tmp_expr = py_expr!("{tmp:id}", tmp = tmp_name);

    blocks.push(compat_block_from_blockpy(
        assign_label.clone(),
        assign_body,
        BlockPyTerm::Jump(BlockPyLabel::from(body_entry)),
    ));

    let exhausted_test = py_expr!(
        "__dp_is_({value:expr}, __dp__.ITER_COMPLETE)",
        value = tmp_expr
    );
    let check_body = if is_async {
        Vec::new()
    } else {
        vec![py_stmt!(
            "{tmp:id} = __dp_next_or_sentinel({iter:expr})",
            tmp = tmp_name,
            iter = iter_expr.clone(),
        )]
    };
    blocks.push(compat_if_jump_block(
        loop_check_label.clone(),
        check_body,
        exhausted_test,
        exhausted_entry,
        assign_label,
    ));

    let mut setup_body = linear;
    if is_async {
        setup_body.push(py_stmt!(
            "{iter:id} = __dp_aiter({iterable:expr})",
            iter = iter_name,
            iterable = iterable,
        ));
    } else {
        setup_body.push(py_stmt!(
            "{iter:id} = __dp_iter({iterable:expr})",
            iter = iter_name,
            iterable = iterable,
        ));
    }
    blocks.push(compat_block_from_blockpy(
        setup_label.clone(),
        setup_body,
        BlockPyTerm::Jump(BlockPyLabel::from(loop_continue_label)),
    ));
    setup_label
}

pub(crate) fn lower_for_loop_continue_entry_with_state(
    blocks: &mut Vec<BlockPyBlock>,
    fn_name: &str,
    iter_name: &str,
    tmp_name: &str,
    loop_check_label: String,
    is_async: bool,
    try_regions: &mut Vec<TryRegionPlan>,
    mut state: GeneratorStmtSequenceLoweringState,
) -> (String, GeneratorStmtSequenceLoweringState) {
    let entry = if is_async {
        build_async_for_continue_entry(
            blocks,
            fn_name,
            py_expr!("{iter:id}", iter = iter_name),
            tmp_name,
            loop_check_label.as_str(),
            state.closure_state,
            try_regions,
            &mut state.resume_order,
            &mut state.yield_sites,
            &mut state.next_block_id,
        )
    } else {
        loop_check_label
    };
    (entry, state)
}

fn compat_next_temp(prefix: &str, next_id: &mut usize) -> String {
    let current = *next_id;
    *next_id += 1;
    format!("_dp_{prefix}_{current}")
}

fn compat_sanitize_ident(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn compat_next_label(fn_name: &str, next_id: &mut usize) -> String {
    let current = *next_id;
    *next_id += 1;
    format!("_dp_bb_{}_{}", compat_sanitize_ident(fn_name), current)
}

#[derive(Debug, Clone)]
pub(crate) struct TryPlan {
    pub except_exc_name: String,
    pub finally_reason_name: Option<String>,
    pub finally_return_value_name: Option<String>,
    pub finally_dispatch_label: Option<String>,
    pub finally_return_label: Option<String>,
    pub finally_exc_name: Option<String>,
}

pub(crate) fn build_try_plan(
    fn_name: &str,
    has_finally: bool,
    needs_finally_return_flow: bool,
    next_id: &mut usize,
) -> TryPlan {
    let except_exc_name = compat_next_temp("try_exc", next_id);
    let finally_reason_name = if has_finally && needs_finally_return_flow {
        Some(compat_next_temp("try_reason", next_id))
    } else {
        None
    };
    let finally_return_value_name = if has_finally && needs_finally_return_flow {
        Some(compat_next_temp("try_value", next_id))
    } else {
        None
    };
    let finally_dispatch_label = if has_finally && needs_finally_return_flow {
        Some(compat_next_label(fn_name, next_id))
    } else {
        None
    };
    let finally_return_label = if has_finally && needs_finally_return_flow {
        Some(compat_next_label(fn_name, next_id))
    } else {
        None
    };
    let finally_exc_name = if has_finally {
        Some(compat_next_temp("try_exc", next_id))
    } else {
        None
    };

    TryPlan {
        except_exc_name,
        finally_reason_name,
        finally_return_value_name,
        finally_dispatch_label,
        finally_return_label,
        finally_exc_name,
    }
}

impl TryPlan {
    pub(crate) fn finally_cont_label(&self, rest_entry: &str) -> String {
        self.finally_dispatch_label
            .clone()
            .unwrap_or_else(|| rest_entry.to_string())
    }
}

pub(crate) fn prepare_finally_body(
    finalbody: &ast::StmtBody,
    finally_exc_name: Option<&str>,
) -> Vec<Box<Stmt>> {
    let mut finally_body = flatten_stmt_boxes(&finalbody.body);
    if let Some(finally_exc_name) = finally_exc_name {
        finally_body.insert(
            0,
            Box::new(py_stmt!(
                "{exc:id} = __dp_current_exception()",
                exc = finally_exc_name,
            )),
        );
        finally_body.push(Box::new(py_stmt!(
            "if __dp_is_not({exc:id}, None):\n    raise {exc:id}",
            exc = finally_exc_name,
        )));
    }
    finally_body
}

pub(crate) fn prepare_except_body(handlers: &[ast::ExceptHandler]) -> Vec<Box<Stmt>> {
    handlers
        .first()
        .map(|handler| {
            let ast::ExceptHandler::ExceptHandler(handler) = handler;
            flatten_stmt_boxes(&handler.body.body)
        })
        .unwrap_or_else(|| vec![Box::new(py_stmt!("raise"))])
}

pub(crate) struct LoweredTryRegions {
    pub body_label: String,
    pub except_label: String,
    pub body_region_range: std::ops::Range<usize>,
    pub else_region_range: std::ops::Range<usize>,
    pub except_region_range: Option<std::ops::Range<usize>>,
    pub finally_region_range: Option<std::ops::Range<usize>>,
    pub finally_label: Option<String>,
    pub finally_normal_entry: Option<String>,
}

pub(crate) fn lower_try_regions<F>(
    blocks: &mut Vec<BlockPyBlock>,
    try_plan: &TryPlan,
    rest_entry: &str,
    finally_body: Option<Vec<Box<Stmt>>>,
    else_body: Vec<Box<Stmt>>,
    try_body: Vec<Box<Stmt>>,
    except_body: Option<Vec<Box<Stmt>>>,
    lower_region: &mut F,
) -> LoweredTryRegions
where
    F: FnMut(&[Box<Stmt>], String, &mut Vec<BlockPyBlock>) -> String,
{
    let finally_label = if let Some(finally_body) = finally_body {
        let finally_region_start = blocks.len();
        let finally_label = lower_region(
            &finally_body,
            try_plan.finally_cont_label(rest_entry),
            blocks,
        );
        let finally_region_end = blocks.len();
        let finally_normal_entry = try_plan.finally_exc_name.as_ref().map(|finally_exc_name| {
            let normal_label = format!("{finally_label}__normal");
            blocks.push(compat_block_from_blockpy(
                normal_label.clone(),
                vec![py_stmt!("{exc:id} = None", exc = finally_exc_name.as_str(),)],
                BlockPyTerm::Jump(BlockPyLabel::from(finally_label.clone())),
            ));
            normal_label
        });
        if let (
            Some(finally_return_label),
            Some(finally_dispatch_label),
            Some(return_name),
            Some(reason_name),
        ) = (
            try_plan.finally_return_label.clone(),
            try_plan.finally_dispatch_label.clone(),
            try_plan.finally_return_value_name.as_ref(),
            try_plan.finally_reason_name.as_ref(),
        ) {
            emit_finally_return_dispatch_blocks(
                blocks,
                finally_return_label,
                finally_dispatch_label,
                return_name,
                reason_name,
                rest_entry.to_string(),
            );
        }
        Some((
            finally_label,
            finally_region_start..finally_region_end,
            finally_normal_entry,
        ))
    } else {
        None
    };

    let cleanup_target = finally_label
        .as_ref()
        .map(|(label, _, normal_entry)| normal_entry.clone().unwrap_or_else(|| label.clone()))
        .unwrap_or_else(|| rest_entry.to_string());

    let else_region_start = blocks.len();
    let else_entry = if else_body.is_empty() {
        cleanup_target.clone()
    } else {
        lower_region(&else_body, cleanup_target.clone(), blocks)
    };
    let else_region_end = blocks.len();

    let except_region_range;
    let except_label = if let Some(except_body) = except_body {
        let except_region_start = blocks.len();
        let except_label = lower_region(&except_body, cleanup_target, blocks);
        let except_region_end = blocks.len();
        except_region_range = Some(except_region_start..except_region_end);
        except_label
    } else if let Some((finally_label, _, _)) = finally_label.clone() {
        except_region_range = None;
        finally_label
    } else {
        panic!("expected except body or finally body when lowering try");
    };

    let body_region_start = blocks.len();
    let body_label = lower_region(&try_body, else_entry, blocks);
    let body_region_end = blocks.len();

    LoweredTryRegions {
        body_label,
        except_label,
        body_region_range: body_region_start..body_region_end,
        else_region_range: else_region_start..else_region_end,
        except_region_range,
        finally_region_range: finally_label.as_ref().map(|(_, range, _)| range.clone()),
        finally_normal_entry: finally_label
            .as_ref()
            .and_then(|(_, _, normal_entry)| normal_entry.clone()),
        finally_label: finally_label.map(|(label, _, _)| label),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn finalize_try_regions(
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    linear: Vec<Stmt>,
    body_label: String,
    except_label: String,
    try_plan: TryPlan,
    body_region_range: std::ops::Range<usize>,
    else_region_range: std::ops::Range<usize>,
    except_region_range: Option<std::ops::Range<usize>>,
    finally_region_range: Option<std::ops::Range<usize>>,
    finally_label: Option<String>,
    finally_normal_entry: Option<String>,
) -> (String, TryRegionPlan) {
    if let (Some(reason_name), Some(return_name), Some(finally_target)) = (
        try_plan.finally_reason_name.as_ref(),
        try_plan.finally_return_value_name.as_ref(),
        finally_normal_entry.as_ref().or(finally_label.as_ref()),
    ) {
        rewrite_region_returns_to_finally_blockpy(
            &mut blocks[body_region_range.clone()],
            reason_name.as_str(),
            return_name.as_str(),
            finally_target.as_str(),
            None,
        );
        rewrite_region_returns_to_finally_blockpy(
            &mut blocks[else_region_range.clone()],
            reason_name.as_str(),
            return_name.as_str(),
            finally_target.as_str(),
            None,
        );
        if let Some(except_region_range) = except_region_range.as_ref() {
            rewrite_region_returns_to_finally_blockpy(
                &mut blocks[except_region_range.clone()],
                reason_name.as_str(),
                return_name.as_str(),
                finally_target.as_str(),
                None,
            );
        }
    }

    if let Some(except_region_range) = except_region_range.as_ref() {
        set_region_exc_param(
            blocks,
            except_region_range,
            try_plan.except_exc_name.as_str(),
        );
    }
    if let (Some(finally_region_range), Some(finally_exc_name)) = (
        finally_region_range.as_ref(),
        try_plan.finally_exc_name.as_ref(),
    ) {
        set_region_exc_param(blocks, finally_region_range, finally_exc_name.as_str());
    }

    let cleanup_region_labels = if finally_label.is_some() {
        let mut labels = collect_region_label_names(&blocks[else_region_range.clone()]);
        if let Some(except_region_range) = except_region_range.as_ref() {
            labels.extend(collect_region_label_names(
                &blocks[except_region_range.clone()],
            ));
        }
        labels
    } else {
        Vec::new()
    };
    let try_region = TryRegionPlan {
        body_region_labels: collect_region_label_names(&blocks[body_region_range]),
        body_exception_target: except_label.clone(),
        cleanup_region_labels,
        cleanup_exception_target: finally_label.clone(),
    };

    let label = emit_try_jump_entry(blocks, label, linear, body_label, except_label);
    (label, try_region)
}

pub(crate) fn emit_finally_return_dispatch_blocks(
    blocks: &mut Vec<BlockPyBlock>,
    finally_return_label: String,
    finally_dispatch_label: String,
    return_name: &str,
    reason_name: &str,
    rest_entry: String,
) {
    blocks.push(compat_block_from_blockpy(
        finally_return_label.clone(),
        Vec::new(),
        BlockPyTerm::Return(Some(py_expr!("{name:id}", name = return_name).into())),
    ));
    blocks.push(compat_block_from_blockpy(
        finally_dispatch_label.clone(),
        Vec::new(),
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test: py_expr!("__dp_eq({reason:id}, 'return')", reason = reason_name,).into(),
            body: Box::new(BlockPyBlock {
                label: BlockPyLabel::from(format!("{finally_dispatch_label}_then")),
                exc_param: None,
                body: Vec::new(),
                term: BlockPyTerm::Jump(BlockPyLabel::from(finally_return_label)),
            }),
            orelse: Box::new(BlockPyBlock {
                label: BlockPyLabel::from(format!("{finally_dispatch_label}_else")),
                exc_param: None,
                body: Vec::new(),
                term: BlockPyTerm::Jump(BlockPyLabel::from(rest_entry)),
            }),
        }),
    ));
}

pub(crate) fn collect_region_label_names(blocks: &[BlockPyBlock]) -> Vec<String> {
    blocks
        .iter()
        .map(|block| block.label.as_str().to_string())
        .collect()
}

pub(crate) fn emit_try_jump_entry(
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    linear: Vec<Stmt>,
    body_label: String,
    except_label: String,
) -> String {
    blocks.push(compat_block_from_blockpy(
        label.clone(),
        linear,
        BlockPyTerm::TryJump(BlockPyTryJump {
            body_label: BlockPyLabel::from(body_label),
            except_label: BlockPyLabel::from(except_label),
        }),
    ));
    label
}

fn block_references_label(block: &BlockPyBlock, label: &str) -> bool {
    fn stmt_references_label(stmt: &BlockPyStmt, label: &str) -> bool {
        match stmt {
            BlockPyStmt::If(if_stmt) => {
                stmt_list_references_label(&if_stmt.body, label)
                    || stmt_list_references_label(&if_stmt.orelse, label)
            }
            BlockPyStmt::Jump(target) => target.as_str() == label,
            BlockPyStmt::BranchTable(branch) => {
                branch.default_label.as_str() == label
                    || branch.targets.iter().any(|target| target.as_str() == label)
            }
            BlockPyStmt::TryJump(try_jump) => {
                try_jump.body_label.as_str() == label || try_jump.except_label.as_str() == label
            }
            _ => false,
        }
    }

    fn stmt_list_references_label(stmts: &[BlockPyStmt], label: &str) -> bool {
        stmts.iter().any(|stmt| stmt_references_label(stmt, label))
    }

    block
        .body
        .iter()
        .any(|stmt| stmt_references_label(stmt, label))
        || match &block.term {
            BlockPyTerm::Jump(target) => target.as_str() == label,
            BlockPyTerm::IfTerm(if_term) => {
                block_references_label(&if_term.body, label)
                    || block_references_label(&if_term.orelse, label)
                    || if_term.body.label.as_str() == label
                    || if_term.orelse.label.as_str() == label
            }
            BlockPyTerm::BranchTable(branch) => {
                branch.default_label.as_str() == label
                    || branch.targets.iter().any(|target| target.as_str() == label)
            }
            BlockPyTerm::TryJump(try_jump) => {
                try_jump.body_label.as_str() == label || try_jump.except_label.as_str() == label
            }
            BlockPyTerm::Raise(_) | BlockPyTerm::Return(_) => false,
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

pub fn rewrite_ast_to_blockpy_module(
    module: &StmtBody,
    function_identity_by_node: &FunctionIdentityByNode,
) -> Result<BlockPyModule, String> {
    let mut module_out = BlockPyModule {
        functions: Vec::new(),
        module_init: None,
    };

    for stmt in &module.body {
        match stmt.as_ref() {
            Stmt::FunctionDef(func) => {
                lower_function_recursive(
                    func,
                    function_identity_by_node,
                    &mut module_out.functions,
                    &mut module_out.module_init,
                )?;
            }
            stmt if is_ignorable_module_stmt(stmt) => {}
            other => {
                return Err(format!(
                    "rewrite_ast_to_blockpy_module expects module-init-wrapped input; found top-level {}:\n{}",
                    stmt_kind_name(other),
                    ruff_ast_to_string(other).trim_end()
                ));
            }
        }
    }
    if module_out.module_init.is_none() {
        return Err(
            "rewrite_ast_to_blockpy_module expects wrapped-module input with `_dp_module_init`"
                .to_string(),
        );
    }

    validate_final_blockpy_module(&module_out);

    Ok(module_out)
}

fn validate_final_blockpy_module(module: &BlockPyModule) {
    for function in &module.functions {
        for block in &function.blocks {
            for stmt in &block.body {
                validate_no_live_yield_in_stmt(stmt);
            }
        }
    }
}

fn validate_no_live_yield_in_stmt(stmt: &BlockPyStmt) {
    match stmt {
        BlockPyStmt::Assign(assign) => validate_no_live_yield_in_expr(&assign.value),
        BlockPyStmt::Expr(expr) => validate_no_live_yield_in_expr(expr),
        BlockPyStmt::If(if_stmt) => {
            validate_no_live_yield_in_expr(&if_stmt.test);
            for stmt in &if_stmt.body {
                validate_no_live_yield_in_stmt(stmt);
            }
            for stmt in &if_stmt.orelse {
                validate_no_live_yield_in_stmt(stmt);
            }
        }
        BlockPyStmt::BranchTable(branch) => validate_no_live_yield_in_expr(&branch.index),
        BlockPyStmt::Return(value) => {
            if let Some(value) = value {
                validate_no_live_yield_in_expr(value);
            }
        }
        BlockPyStmt::Raise(raise_stmt) => {
            if let Some(exc) = &raise_stmt.exc {
                validate_no_live_yield_in_expr(exc);
            }
        }
        BlockPyStmt::Pass
        | BlockPyStmt::Delete(_)
        | BlockPyStmt::FunctionDef(_)
        | BlockPyStmt::Jump(_)
        | BlockPyStmt::TryJump(_) => {}
    }
}

fn validate_no_live_yield_in_expr(expr: &crate::basic_block::block_py::BlockPyExpr) {
    #[derive(Default)]
    struct YieldProbe {
        has_yield: bool,
    }

    impl Transformer for YieldProbe {
        fn visit_expr(&mut self, expr: &mut Expr) {
            match expr {
                Expr::Yield(_) | Expr::YieldFrom(_) => self.has_yield = true,
                _ => walk_expr(self, expr),
            }
        }
    }

    let mut probe = YieldProbe::default();
    let mut expr = expr.to_expr();
    probe.visit_expr(&mut expr);
    assert!(
        !probe.has_yield,
        "Yield/YieldFrom should be lowered before final BlockPy emission"
    );
}

fn lower_function_recursive(
    func: &ast::StmtFunctionDef,
    function_identity_by_node: &FunctionIdentityByNode,
    out: &mut Vec<BlockPyFunction>,
    module_init: &mut Option<String>,
) -> Result<(), String> {
    let Some((bind_name, _display_name, qualname, binding_target)) = function_identity_by_node
        .get(&func.node_index.load())
        .cloned()
    else {
        return Err(format!(
            "missing function identity for function {}\nstmt:\n{}",
            func.name.id,
            ruff_ast_to_string(&Stmt::FunctionDef(func.clone())).trim_end()
        ));
    };

    let lowered = lower_top_level_function(func, bind_name, qualname, binding_target)?;
    if lowered.bind_name == "_dp_module_init" {
        *module_init = Some(lowered.qualname.clone());
    }
    collect_nested_functions_from_blocks(
        &lowered.blocks,
        function_identity_by_node,
        out,
        module_init,
    )?;
    out.push(lowered);
    Ok(())
}

fn collect_nested_functions_from_blocks(
    blocks: &[BlockPyBlock],
    function_identity_by_node: &FunctionIdentityByNode,
    out: &mut Vec<BlockPyFunction>,
    module_init: &mut Option<String>,
) -> Result<(), String> {
    for block in blocks {
        collect_nested_functions_from_stmts(
            &block.body,
            function_identity_by_node,
            out,
            module_init,
        )?;
    }
    Ok(())
}

fn collect_nested_functions_from_stmts(
    stmts: &[BlockPyStmt],
    function_identity_by_node: &FunctionIdentityByNode,
    out: &mut Vec<BlockPyFunction>,
    module_init: &mut Option<String>,
) -> Result<(), String> {
    for stmt in stmts {
        match stmt {
            BlockPyStmt::FunctionDef(func) => {
                lower_function_recursive(func, function_identity_by_node, out, module_init)?;
            }
            BlockPyStmt::If(if_stmt) => {
                collect_nested_functions_from_stmts(
                    &if_stmt.body,
                    function_identity_by_node,
                    out,
                    module_init,
                )?;
                collect_nested_functions_from_stmts(
                    &if_stmt.orelse,
                    function_identity_by_node,
                    out,
                    module_init,
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}

pub(crate) fn lower_stmts_to_blockpy_stmts(stmts: &[Stmt]) -> Result<Vec<BlockPyStmt>, String> {
    let mut out = Vec::new();
    let mut next_label_id = 0usize;
    for stmt in stmts {
        lower_stmt_into(stmt, &mut out, None, &mut next_label_id)?;
    }
    Ok(out)
}

fn generator_stmt_sequence_head(stmt: &Stmt) -> Option<(BlockPyStmt, bool)> {
    let generator_stmt = match lower_stmts_to_blockpy_stmts(std::slice::from_ref(stmt)) {
        Ok(generator_stmt) => generator_stmt,
        Err(err) => {
            return match stmt {
                Stmt::Expr(_) | Stmt::Assign(_) | Stmt::Return(_) => {
                    panic!("failed to convert generator stmt to BlockPy before lowering: {err}")
                }
                _ => None,
            };
        }
    };
    let generator_stmt = generator_stmt
        .into_iter()
        .next()
        .expect("generator stmt conversion should yield one BlockPy stmt");
    match &generator_stmt {
        BlockPyStmt::Expr(BlockPyExpr::Yield(_))
        | BlockPyStmt::Expr(BlockPyExpr::YieldFrom(_))
        | BlockPyStmt::Assign(BlockPyAssign {
            value: BlockPyExpr::Yield(_),
            ..
        })
        | BlockPyStmt::Assign(BlockPyAssign {
            value: BlockPyExpr::YieldFrom(_),
            ..
        })
        | BlockPyStmt::Return(Some(BlockPyExpr::Yield(_)))
        | BlockPyStmt::Return(Some(BlockPyExpr::YieldFrom(_))) => {}
        _ => return None,
    }
    let needs_rest_entry = blockpy_stmt_requires_generator_rest_entry(&generator_stmt);
    Some((generator_stmt, needs_rest_entry))
}

#[derive(Clone)]
pub(crate) struct GeneratorStmtSequencePlan {
    generator_stmt: BlockPyStmt,
    pub needs_rest_entry: bool,
}

pub(crate) fn plan_generator_stmt_in_sequence(stmt: &Stmt) -> Option<GeneratorStmtSequencePlan> {
    let (generator_stmt, needs_rest_entry) = generator_stmt_sequence_head(stmt)?;
    Some(GeneratorStmtSequencePlan {
        generator_stmt,
        needs_rest_entry,
    })
}

pub(crate) fn plan_stmt_sequence_head(
    stmt: &Stmt,
    allow_generator_head: bool,
) -> StmtSequenceHeadPlan {
    if allow_generator_head {
        match stmt {
            Stmt::Expr(_) | Stmt::Assign(_) | Stmt::Return(_) => {
                if let Some(plan) = plan_generator_stmt_in_sequence(stmt) {
                    return StmtSequenceHeadPlan::Generator {
                        plan,
                        sync_target_cells: matches!(stmt, Stmt::Assign(_)),
                    };
                }
            }
            _ => {}
        }
    }

    match stmt {
        Stmt::Expr(_) | Stmt::Pass(_) | Stmt::Assign(_) | Stmt::Global(_) | Stmt::Nonlocal(_) => {
            StmtSequenceHeadPlan::Linear(stmt.clone())
        }
        Stmt::FunctionDef(func_def) => StmtSequenceHeadPlan::FunctionDef(func_def.clone()),
        Stmt::Raise(raise_stmt) => StmtSequenceHeadPlan::Raise(raise_stmt.clone()),
        Stmt::Delete(delete_stmt) => StmtSequenceHeadPlan::Delete(delete_stmt.clone()),
        Stmt::Return(ret) => {
            StmtSequenceHeadPlan::Return(ret.value.as_ref().map(|expr| *expr.clone()))
        }
        Stmt::If(if_stmt) => StmtSequenceHeadPlan::If(if_stmt.clone()),
        Stmt::While(while_stmt) => StmtSequenceHeadPlan::While(while_stmt.clone()),
        Stmt::For(for_stmt) => StmtSequenceHeadPlan::For(for_stmt.clone()),
        Stmt::Try(try_stmt) => StmtSequenceHeadPlan::Try(try_stmt.clone()),
        Stmt::With(with_stmt) => StmtSequenceHeadPlan::With(with_stmt.clone()),
        Stmt::Break(_) => StmtSequenceHeadPlan::Break,
        Stmt::Continue(_) => StmtSequenceHeadPlan::Continue,
        _ => StmtSequenceHeadPlan::Unsupported,
    }
}

pub(crate) fn drive_stmt_sequence_until_control<FDef, FDelete>(
    stmts: &[Box<Stmt>],
    mut linear: Vec<Stmt>,
    allow_generator_head: bool,
    lower_non_bb_def: &mut FDef,
    rewrite_delete: &mut FDelete,
) -> StmtSequenceDriveResult
where
    FDef: FnMut(&ast::StmtFunctionDef) -> Vec<Stmt>,
    FDelete: FnMut(&ast::StmtDelete) -> Vec<Stmt>,
{
    let mut index = 0;
    while index < stmts.len() {
        match plan_stmt_sequence_head(stmts[index].as_ref(), allow_generator_head) {
            StmtSequenceHeadPlan::Linear(stmt) => {
                linear.push(stmt);
                index += 1;
            }
            StmtSequenceHeadPlan::FunctionDef(func_def) => {
                if func_def.name.id.as_str().starts_with("_dp_bb_") {
                    linear.push(Stmt::FunctionDef(func_def));
                } else {
                    linear.extend(lower_non_bb_def(&func_def));
                }
                index += 1;
            }
            StmtSequenceHeadPlan::Delete(delete_stmt) => {
                linear.extend(rewrite_delete(&delete_stmt));
                index += 1;
            }
            plan => {
                return StmtSequenceDriveResult::Break {
                    linear,
                    index,
                    plan,
                }
            }
        }
    }
    StmtSequenceDriveResult::Exhausted { linear }
}

fn compat_blockpy_raise_from_stmt(raise_stmt: ast::StmtRaise) -> BlockPyRaise {
    assert!(
        raise_stmt.cause.is_none(),
        "raise-from should be lowered before Ruff AST -> BlockPy conversion"
    );
    BlockPyRaise {
        exc: raise_stmt.exc.map(|expr| (*expr).into()),
    }
}

pub(crate) fn lower_common_stmt_sequence_head<FSeq>(
    plan: StmtSequenceHeadPlan,
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    next_label: &mut dyn FnMut() -> String,
    break_label: Option<String>,
    continue_label: Option<String>,
    lower_sequence: &mut FSeq,
) -> Option<String>
where
    FSeq: FnMut(&[Box<Stmt>], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    match plan {
        StmtSequenceHeadPlan::Raise(raise_stmt) => Some(emit_sequence_raise_block(
            blocks,
            next_label(),
            linear,
            compat_blockpy_raise_from_stmt(raise_stmt),
        )),
        StmtSequenceHeadPlan::Return(value) => Some(emit_sequence_return_block(
            blocks,
            next_label(),
            linear,
            value,
        )),
        StmtSequenceHeadPlan::If(if_stmt) => Some(lower_if_stmt_sequence_from_stmt(
            if_stmt,
            remaining_stmts,
            cont_label,
            linear,
            blocks,
            next_label(),
            &mut |stmts, cont_label, blocks| lower_sequence(stmts, cont_label, None, blocks),
        )),
        StmtSequenceHeadPlan::While(while_stmt) => {
            let test_label = next_label();
            let linear_label = if linear.is_empty() {
                None
            } else {
                Some(next_label())
            };
            Some(lower_while_stmt_sequence_from_stmt(
                while_stmt,
                remaining_stmts,
                cont_label,
                linear,
                blocks,
                test_label,
                linear_label,
                lower_sequence,
            ))
        }
        StmtSequenceHeadPlan::With(with_stmt) => {
            let jump_label = if linear.is_empty() {
                None
            } else {
                Some(next_label())
            };
            Some(lower_with_stmt_sequence(
                with_stmt,
                remaining_stmts,
                cont_label,
                linear,
                blocks,
                jump_label,
                &mut |stmts, cont_label, blocks| lower_sequence(stmts, cont_label, None, blocks),
            ))
        }
        StmtSequenceHeadPlan::Break => match break_label {
            Some(break_label) => Some(emit_sequence_jump_block(
                blocks,
                next_label(),
                linear,
                break_label,
            )),
            None => Some(cont_label),
        },
        StmtSequenceHeadPlan::Continue => match continue_label {
            Some(continue_label) => Some(emit_sequence_jump_block(
                blocks,
                next_label(),
                linear,
                continue_label,
            )),
            None => Some(cont_label),
        },
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_for_stmt_sequence_head<F>(
    fn_name: &str,
    for_stmt: ast::StmtFor,
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    iter_name: &str,
    tmp_name: &str,
    loop_check_label: String,
    loop_continue_label: String,
    assign_body: Vec<Stmt>,
    next_block_id: &Cell<usize>,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Box<Stmt>], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let mut next_id = next_block_id.get();
    let assign_label = compat_next_label(fn_name, &mut next_id);
    let setup_label = compat_next_label(fn_name, &mut next_id);
    next_block_id.set(next_id);
    lower_for_stmt_sequence(
        for_stmt,
        remaining_stmts,
        cont_label,
        linear,
        blocks,
        iter_name,
        tmp_name,
        loop_check_label,
        loop_continue_label,
        assign_label,
        setup_label,
        assign_body,
        lower_region,
    )
}

pub(crate) fn lower_generator_stmt_sequence_plan(
    plan: &GeneratorStmtSequencePlan,
    linear: Vec<Stmt>,
    rest_entry: Option<String>,
    blocks: &mut Vec<BlockPyBlock>,
    closure_state: bool,
    try_regions: &mut Vec<TryRegionPlan>,
    resume_order: &mut Vec<String>,
    yield_sites: &mut Vec<GeneratorYieldSite>,
    next_block_id: &mut usize,
    fn_name: &str,
    cell_slots: Option<&std::collections::HashSet<String>>,
) -> Option<String> {
    lower_generator_blockpy_stmt_in_sequence(
        &plan.generator_stmt,
        linear,
        rest_entry,
        blocks,
        None,
        closure_state,
        try_regions,
        resume_order,
        yield_sites,
        next_block_id,
        fn_name,
        cell_slots,
    )
}

pub(crate) fn lower_generator_stmt_sequence_head<F>(
    fn_name: &str,
    plan: GeneratorStmtSequencePlan,
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    mut state: GeneratorStmtSequenceLoweringState,
    try_regions: &mut Vec<TryRegionPlan>,
    cell_slots: Option<&std::collections::HashSet<String>>,
    lower_rest: &mut F,
) -> (Option<String>, GeneratorStmtSequenceLoweringState)
where
    F: FnMut(
        &[Box<Stmt>],
        String,
        &mut Vec<BlockPyBlock>,
        GeneratorStmtSequenceLoweringState,
    ) -> (String, GeneratorStmtSequenceLoweringState),
{
    let rest_entry = if plan.needs_rest_entry {
        let (entry, updated_state) = lower_rest(remaining_stmts, cont_label, blocks, state);
        state = updated_state;
        Some(entry)
    } else {
        None
    };
    let label = lower_generator_stmt_sequence_plan(
        &plan,
        linear,
        rest_entry,
        blocks,
        state.closure_state,
        try_regions,
        &mut state.resume_order,
        &mut state.yield_sites,
        &mut state.next_block_id,
        fn_name,
        cell_slots,
    );
    (label, state)
}

pub(crate) fn lower_stmt_sequence_with_state<FDef, FTemp>(
    fn_name: &str,
    stmts: &[Box<Stmt>],
    cont_label: String,
    break_label: Option<String>,
    continue_label: Option<String>,
    blocks: &mut Vec<BlockPyBlock>,
    cell_slots: &HashSet<String>,
    generator_state: &mut BlockPySequenceGeneratorState,
    try_regions: &mut Vec<TryRegionPlan>,
    next_block_id: &mut usize,
    lower_non_bb_def: &mut FDef,
    next_temp: &mut FTemp,
) -> String
where
    FDef: FnMut(&ast::StmtFunctionDef) -> Vec<Stmt>,
    FTemp: FnMut(&str, &mut usize) -> String,
{
    if stmts.is_empty() {
        return cont_label;
    }

    let mut linear = Vec::new();
    let mut index = 0;
    while index < stmts.len() {
        let plan;
        (linear, index, plan) = match drive_stmt_sequence_until_control(
            &stmts[index..],
            linear,
            generator_state.enabled,
            lower_non_bb_def,
            &mut rewrite_delete_to_deleted_sentinel,
        ) {
            StmtSequenceDriveResult::Exhausted { linear } => {
                let label = compat_next_label(fn_name, next_block_id);
                return emit_sequence_jump_block(blocks, label, linear, cont_label);
            }
            StmtSequenceDriveResult::Break {
                linear,
                index: break_index,
                plan,
            } => (linear, index + break_index, plan),
        };

        match plan {
            StmtSequenceHeadPlan::Generator {
                plan,
                sync_target_cells,
            } => {
                let initial_state = GeneratorStmtSequenceLoweringState {
                    enabled: generator_state.enabled,
                    closure_state: generator_state.closure_state,
                    resume_order: std::mem::take(&mut generator_state.resume_order),
                    yield_sites: std::mem::take(&mut generator_state.yield_sites),
                    next_block_id: *next_block_id,
                };
                let mut local_try_regions = Vec::new();
                let (label, updated_state) = lower_generator_stmt_sequence_head(
                    fn_name,
                    plan,
                    &stmts[index + 1..],
                    cont_label.clone(),
                    linear.clone(),
                    blocks,
                    initial_state,
                    &mut local_try_regions,
                    sync_target_cells.then_some(cell_slots),
                    &mut |stmts, cont_label, blocks, state| {
                        let mut local_generator_state = BlockPySequenceGeneratorState {
                            enabled: state.enabled,
                            closure_state: state.closure_state,
                            resume_order: state.resume_order,
                            yield_sites: state.yield_sites,
                        };
                        let mut local_next_block_id = state.next_block_id;
                        let label = lower_stmt_sequence_with_state(
                            fn_name,
                            stmts,
                            cont_label,
                            break_label.clone(),
                            continue_label.clone(),
                            blocks,
                            cell_slots,
                            &mut local_generator_state,
                            try_regions,
                            &mut local_next_block_id,
                            lower_non_bb_def,
                            next_temp,
                        );
                        (
                            label,
                            GeneratorStmtSequenceLoweringState {
                                enabled: local_generator_state.enabled,
                                closure_state: local_generator_state.closure_state,
                                resume_order: local_generator_state.resume_order,
                                yield_sites: local_generator_state.yield_sites,
                                next_block_id: local_next_block_id,
                            },
                        )
                    },
                );
                try_regions.extend(local_try_regions);
                *next_block_id = updated_state.next_block_id;
                generator_state.resume_order = updated_state.resume_order;
                generator_state.yield_sites = updated_state.yield_sites;
                if let Some(label) = label {
                    return label;
                }
                linear.push(stmts[index].as_ref().clone());
                index += 1;
                continue;
            }
            plan @ (StmtSequenceHeadPlan::Raise(_)
            | StmtSequenceHeadPlan::Return(_)
            | StmtSequenceHeadPlan::If(_)
            | StmtSequenceHeadPlan::While(_)
            | StmtSequenceHeadPlan::With(_)
            | StmtSequenceHeadPlan::Break
            | StmtSequenceHeadPlan::Continue) => {
                let next_id = Cell::new(*next_block_id);
                let label = lower_common_stmt_sequence_head(
                    plan,
                    &stmts[index + 1..],
                    cont_label.clone(),
                    linear,
                    blocks,
                    &mut || {
                        let mut local_next_id = next_id.get();
                        let label = compat_next_label(fn_name, &mut local_next_id);
                        next_id.set(local_next_id);
                        label
                    },
                    break_label.clone(),
                    continue_label.clone(),
                    &mut |stmts, cont_label, loop_break_label, blocks| {
                        if let Some(loop_break_label) = loop_break_label {
                            let mut local_next_id = next_id.get();
                            let label = lower_stmt_sequence_with_state(
                                fn_name,
                                stmts,
                                cont_label.clone(),
                                Some(loop_break_label),
                                Some(cont_label),
                                blocks,
                                cell_slots,
                                generator_state,
                                try_regions,
                                &mut local_next_id,
                                lower_non_bb_def,
                                next_temp,
                            );
                            next_id.set(local_next_id);
                            label
                        } else {
                            let mut local_next_id = next_id.get();
                            let label = lower_stmt_sequence_with_state(
                                fn_name,
                                stmts,
                                cont_label,
                                break_label.clone(),
                                continue_label.clone(),
                                blocks,
                                cell_slots,
                                generator_state,
                                try_regions,
                                &mut local_next_id,
                                lower_non_bb_def,
                                next_temp,
                            );
                            next_id.set(local_next_id);
                            label
                        }
                    },
                );
                *next_block_id = next_id.into_inner();
                if let Some(label) = label {
                    return label;
                }
                unreachable!("common head helper must lower supported head");
            }
            StmtSequenceHeadPlan::For(for_stmt) => {
                let iter_name = next_temp("iter", next_block_id);
                let tmp_name = next_temp("tmp", next_block_id);
                let tmp_expr = py_expr!("{name:id}", name = tmp_name.as_str());
                let loop_check_label = compat_next_label(fn_name, next_block_id);
                let continue_state = GeneratorStmtSequenceLoweringState {
                    enabled: generator_state.enabled,
                    closure_state: generator_state.closure_state,
                    resume_order: std::mem::take(&mut generator_state.resume_order),
                    yield_sites: std::mem::take(&mut generator_state.yield_sites),
                    next_block_id: *next_block_id,
                };
                let (loop_continue_label, continue_state) =
                    lower_for_loop_continue_entry_with_state(
                        blocks,
                        fn_name,
                        iter_name.as_str(),
                        tmp_name.as_str(),
                        loop_check_label.clone(),
                        for_stmt.is_async,
                        try_regions,
                        continue_state,
                    );
                *next_block_id = continue_state.next_block_id;
                generator_state.resume_order = continue_state.resume_order;
                generator_state.yield_sites = continue_state.yield_sites;
                let assign_body = build_for_target_assign_body(
                    for_stmt.target.as_ref(),
                    tmp_expr,
                    tmp_name.as_str(),
                    cell_slots,
                    &mut |prefix| next_temp(prefix, next_block_id),
                );
                let next_id = Cell::new(*next_block_id);
                let label = lower_for_stmt_sequence_head(
                    fn_name,
                    for_stmt,
                    &stmts[index + 1..],
                    cont_label.clone(),
                    linear,
                    blocks,
                    iter_name.as_str(),
                    tmp_name.as_str(),
                    loop_check_label,
                    loop_continue_label,
                    assign_body,
                    &next_id,
                    &mut |stmts, cont_label, loop_break_label, blocks| {
                        if let Some(loop_break_label) = loop_break_label {
                            let mut local_next_id = next_id.get();
                            let label = lower_stmt_sequence_with_state(
                                fn_name,
                                stmts,
                                cont_label.clone(),
                                Some(loop_break_label),
                                Some(cont_label),
                                blocks,
                                cell_slots,
                                generator_state,
                                try_regions,
                                &mut local_next_id,
                                lower_non_bb_def,
                                next_temp,
                            );
                            next_id.set(local_next_id);
                            label
                        } else {
                            let mut local_next_id = next_id.get();
                            let label = lower_stmt_sequence_with_state(
                                fn_name,
                                stmts,
                                cont_label,
                                break_label.clone(),
                                continue_label.clone(),
                                blocks,
                                cell_slots,
                                generator_state,
                                try_regions,
                                &mut local_next_id,
                                lower_non_bb_def,
                                next_temp,
                            );
                            next_id.set(local_next_id);
                            label
                        }
                    },
                );
                *next_block_id = next_id.into_inner();
                return label;
            }
            StmtSequenceHeadPlan::Try(try_stmt) => {
                let next_id = Cell::new(*next_block_id);
                let label = if try_stmt.is_star {
                    let mut local_next_id = next_id.get();
                    let jump_label = (!linear.is_empty())
                        .then(|| compat_next_label(fn_name, &mut local_next_id));
                    next_id.set(local_next_id);
                    lower_star_try_stmt_sequence(
                        try_stmt,
                        &stmts[index + 1..],
                        cont_label.clone(),
                        linear,
                        blocks,
                        jump_label,
                        &mut |stmts, cont_label, blocks| {
                            let mut local_next_id = next_id.get();
                            let label = lower_stmt_sequence_with_state(
                                fn_name,
                                stmts,
                                cont_label,
                                break_label.clone(),
                                continue_label.clone(),
                                blocks,
                                cell_slots,
                                generator_state,
                                try_regions,
                                &mut local_next_id,
                                lower_non_bb_def,
                                next_temp,
                            );
                            next_id.set(local_next_id);
                            label
                        },
                    )
                } else {
                    let mut local_next_id = next_id.get();
                    let has_finally = !try_stmt.finalbody.body.is_empty();
                    let needs_finally_return_flow = has_finally
                        && (contains_return_stmt_in_body(&try_stmt.body.body)
                            || contains_return_stmt_in_handlers(&try_stmt.handlers)
                            || contains_return_stmt_in_body(&try_stmt.orelse.body));
                    let try_plan = build_try_plan(
                        fn_name,
                        has_finally,
                        needs_finally_return_flow,
                        &mut local_next_id,
                    );
                    let label = compat_next_label(fn_name, &mut local_next_id);
                    next_id.set(local_next_id);
                    let (entry, try_region) = lower_try_stmt_sequence(
                        try_stmt,
                        &stmts[index + 1..],
                        cont_label.clone(),
                        linear,
                        blocks,
                        label.clone(),
                        try_plan,
                        &mut |stmts, cont_label, blocks| {
                            let mut local_next_id = next_id.get();
                            let label = lower_stmt_sequence_with_state(
                                fn_name,
                                stmts,
                                cont_label,
                                break_label.clone(),
                                continue_label.clone(),
                                blocks,
                                cell_slots,
                                generator_state,
                                try_regions,
                                &mut local_next_id,
                                lower_non_bb_def,
                                next_temp,
                            );
                            next_id.set(local_next_id);
                            label
                        },
                    );
                    try_regions.push(try_region);
                    entry
                };
                *next_block_id = next_id.into_inner();
                return label;
            }
            StmtSequenceHeadPlan::Linear(_)
            | StmtSequenceHeadPlan::FunctionDef(_)
            | StmtSequenceHeadPlan::Delete(_) => {
                unreachable!("sequence driver should consume linear/functiondef/delete heads")
            }
            StmtSequenceHeadPlan::Unsupported => return cont_label,
        }
    }

    let label = compat_next_label(fn_name, next_block_id);
    emit_sequence_jump_block(blocks, label, linear, cont_label)
}

pub(crate) fn lower_expanded_stmt_sequence<F>(
    desugared_stmt: Stmt,
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    jump_label: Option<String>,
    lower_sequence: &mut F,
) -> String
where
    F: FnMut(&[Box<Stmt>], String, &mut Vec<BlockPyBlock>) -> String,
{
    let mut expanded = match desugared_stmt {
        Stmt::BodyStmt(body) => body.body,
        stmt => vec![Box::new(stmt)],
    };
    expanded.extend(remaining_stmts.iter().cloned());
    let expanded_entry = lower_sequence(&expanded, cont_label, blocks);
    if linear.is_empty() {
        return expanded_entry;
    }
    let jump_label = jump_label.expect("linear prefix requires a jump label");
    blocks.push(compat_block_from_blockpy(
        jump_label.clone(),
        linear,
        BlockPyTerm::Jump(BlockPyLabel::from(expanded_entry)),
    ));
    jump_label
}

pub(crate) fn lower_if_stmt_sequence<F>(
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    linear: Vec<Stmt>,
    test: Expr,
    then_body: &[Box<Stmt>],
    else_body: &[Box<Stmt>],
    rest_entry: String,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Box<Stmt>], String, &mut Vec<BlockPyBlock>) -> String,
{
    let then_entry = lower_region(then_body, rest_entry.clone(), blocks);
    let else_entry = lower_region(else_body, rest_entry, blocks);
    emit_if_branch_block(blocks, label, linear, test, then_entry, else_entry)
}

pub(crate) fn lower_if_stmt_sequence_from_stmt<F>(
    if_stmt: ast::StmtIf,
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Box<Stmt>], String, &mut Vec<BlockPyBlock>) -> String,
{
    let then_body = flatten_stmt_boxes(&if_stmt.body.body);
    let else_body = flatten_stmt_boxes(&extract_if_else_body(&if_stmt));
    let rest_entry = lower_region(remaining_stmts, cont_label, blocks);
    lower_if_stmt_sequence(
        blocks,
        label,
        linear,
        *if_stmt.test,
        &then_body,
        &else_body,
        rest_entry,
        lower_region,
    )
}

fn extract_if_else_body(if_stmt: &ast::StmtIf) -> Vec<Box<Stmt>> {
    if if_stmt.elif_else_clauses.is_empty() {
        return Vec::new();
    }
    if_stmt
        .elif_else_clauses
        .first()
        .map(|clause| clause.body.body.clone())
        .unwrap_or_default()
}

pub(crate) fn lower_while_stmt_sequence<F>(
    blocks: &mut Vec<BlockPyBlock>,
    test_label: String,
    linear_label: Option<String>,
    linear: Vec<Stmt>,
    test: Expr,
    body: &[Box<Stmt>],
    else_body: &[Box<Stmt>],
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Box<Stmt>], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let rest_entry = lower_region(remaining_stmts, cont_label, None, blocks);
    let cond_false_entry = if else_body.is_empty() {
        rest_entry.clone()
    } else {
        lower_region(else_body, rest_entry.clone(), None, blocks)
    };
    let body_entry = lower_region(body, test_label.clone(), Some(rest_entry), blocks);
    emit_simple_while_blocks(
        blocks,
        test_label,
        linear_label,
        linear,
        test,
        body_entry,
        cond_false_entry,
    )
}

pub(crate) fn lower_while_stmt_sequence_from_stmt<F>(
    while_stmt: ast::StmtWhile,
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    test_label: String,
    linear_label: Option<String>,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Box<Stmt>], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let body = flatten_stmt_boxes(&while_stmt.body.body);
    let else_body = flatten_stmt_boxes(&while_stmt.orelse.body);
    lower_while_stmt_sequence(
        blocks,
        test_label,
        linear_label,
        linear,
        *while_stmt.test,
        &body,
        &else_body,
        remaining_stmts,
        cont_label,
        lower_region,
    )
}

pub(crate) fn lower_for_stmt_exit_entries<F>(
    blocks: &mut Vec<BlockPyBlock>,
    else_body: &[Box<Stmt>],
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    lower_region: &mut F,
) -> (String, String)
where
    F: FnMut(&[Box<Stmt>], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let rest_entry = lower_region(remaining_stmts, cont_label, None, blocks);
    let exhausted_entry = if else_body.is_empty() {
        rest_entry.clone()
    } else {
        lower_region(else_body, rest_entry.clone(), None, blocks)
    };
    (rest_entry, exhausted_entry)
}

pub(crate) fn lower_for_stmt_body_entry<F>(
    blocks: &mut Vec<BlockPyBlock>,
    loop_continue_label: String,
    body: &[Box<Stmt>],
    break_label: String,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Box<Stmt>], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let body_entry = lower_region(body, loop_continue_label.clone(), Some(break_label), blocks);
    body_entry
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_for_stmt_sequence<F>(
    for_stmt: ast::StmtFor,
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    iter_name: &str,
    tmp_name: &str,
    loop_check_label: String,
    loop_continue_label: String,
    assign_label: String,
    setup_label: String,
    assign_body: Vec<Stmt>,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Box<Stmt>], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let else_body = flatten_stmt_boxes(&for_stmt.orelse.body);
    let (rest_entry, exhausted_entry) = lower_for_stmt_exit_entries(
        blocks,
        &else_body,
        remaining_stmts,
        cont_label,
        lower_region,
    );

    let body = flatten_stmt_boxes(&for_stmt.body.body);
    let body_entry = lower_for_stmt_body_entry(
        blocks,
        loop_continue_label.clone(),
        &body,
        rest_entry.clone(),
        lower_region,
    );

    emit_for_loop_blocks(
        blocks,
        setup_label,
        assign_label,
        loop_check_label,
        loop_continue_label,
        linear,
        iter_name,
        tmp_name,
        *for_stmt.iter,
        for_stmt.is_async,
        exhausted_entry,
        body_entry,
        assign_body,
    )
}

fn lower_top_level_function(
    func: &ast::StmtFunctionDef,
    bind_name: String,
    qualname: String,
    binding_target: BindingTarget,
) -> Result<BlockPyFunction, String> {
    let mut next_label_id = 0usize;
    let kind = function_kind_from_def(func);
    let mut runtime_body = func.body.clone();
    let coroutine_via_generator = matches!(kind, BlockPyFunctionKind::Coroutine);
    if func.is_async {
        lower_coroutine_awaits_to_yield_from(&mut runtime_body.body);
        if coroutine_via_generator && !body_has_yieldlike(&runtime_body) {
            runtime_body
                .body
                .insert(0, coroutine_generator_marker_stmt());
        }
    }
    let has_yield = matches!(
        kind,
        BlockPyFunctionKind::Generator | BlockPyFunctionKind::AsyncGenerator
    ) || coroutine_via_generator;
    let is_generated_genexpr = func.name.id.as_str().contains("_dp_genexpr_");
    let is_generated_comprehension_helper = is_generated_genexpr
        || func.name.id.as_str().contains("_dp_listcomp_")
        || func.name.id.as_str().contains("_dp_setcomp_")
        || func.name.id.as_str().contains("_dp_dictcomp_");
    let is_async_generator_runtime = matches!(kind, BlockPyFunctionKind::AsyncGenerator);
    let is_closure_backed_generator_runtime =
        has_yield && !(is_generated_comprehension_helper && func.is_async);
    let end_label = compat_next_label(func.name.id.as_str(), &mut next_label_id);
    let prepared = lower_function_body_to_blockpy_function(
        func.name.id.as_str(),
        &runtime_body.body,
        bind_name,
        qualname,
        binding_target,
        (*func.parameters).clone(),
        end_label,
        compat_sanitize_ident(func.name.id.as_str()).as_str(),
        has_yield,
        coroutine_via_generator,
        is_async_generator_runtime,
        is_closure_backed_generator_runtime,
        &collect_cell_slots(&runtime_body.body),
        &mut next_label_id,
        &mut |func_def| vec![Stmt::FunctionDef(func_def.clone())],
        &mut compat_next_temp,
    );
    Ok(prepared.function)
}

fn body_has_yieldlike(body: &StmtBody) -> bool {
    let mut probe = YieldLikeProbe::default();
    for stmt in &body.body {
        let mut stmt = stmt.as_ref().clone();
        probe.visit_stmt(&mut stmt);
        if probe.has_yield || probe.has_yield_from {
            return true;
        }
    }
    false
}

fn lower_nested_body_to_stmts(
    body: &StmtBody,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<Vec<BlockPyStmt>, String> {
    let mut out = Vec::new();
    for stmt in &body.body {
        lower_stmt_into(stmt.as_ref(), &mut out, loop_ctx, next_label_id)?;
    }
    Ok(out)
}

fn lower_stmt_into(
    stmt: &Stmt,
    out: &mut Vec<BlockPyStmt>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<(), String> {
    match stmt {
        Stmt::BodyStmt(body) => {
            for stmt in &body.body {
                lower_stmt_into(stmt.as_ref(), out, loop_ctx, next_label_id)?;
            }
        }
        Stmt::Global(_) | Stmt::Nonlocal(_) => {}
        Stmt::Pass(_) => out.push(BlockPyStmt::Pass),
        Stmt::Expr(expr_stmt) => out.push(BlockPyStmt::Expr((*expr_stmt.value).clone().into())),
        Stmt::Assign(assign) => {
            if assign.targets.len() != 1 {
                return Err(assign_delete_error(
                    "multi-target assignment reached BlockPy conversion",
                    stmt,
                ));
            }
            let Some(target) = assign.targets[0].as_name_expr().cloned() else {
                return Err(assign_delete_error(
                    "non-name assignment target reached BlockPy conversion",
                    stmt,
                ));
            };
            out.push(BlockPyStmt::Assign(BlockPyAssign {
                target,
                value: (*assign.value).clone().into(),
            }));
        }
        Stmt::Delete(delete) => {
            if delete.targets.len() != 1 {
                return Err(assign_delete_error(
                    "multi-target delete reached BlockPy conversion",
                    stmt,
                ));
            }
            let Some(target) = delete.targets[0].as_name_expr().cloned() else {
                return Err(assign_delete_error(
                    "non-name delete target reached BlockPy conversion",
                    stmt,
                ));
            };
            out.push(BlockPyStmt::Delete(BlockPyDelete { target }));
        }
        Stmt::FunctionDef(func) => {
            out.push(BlockPyStmt::FunctionDef(func.clone()));
        }
        Stmt::ClassDef(_) => {
            panic!("ClassDef should be lowered before Ruff AST -> BlockPy conversion");
        }
        Stmt::TypeAlias(_) => {
            panic!("TypeAlias should be lowered before Ruff AST -> BlockPy conversion");
        }
        Stmt::AugAssign(_) => {
            panic!("AugAssign should be lowered before Ruff AST -> BlockPy conversion");
        }
        Stmt::AnnAssign(_) => {
            panic!("AnnAssign should be lowered before Ruff AST -> BlockPy conversion");
        }
        Stmt::If(if_stmt) => {
            let body = lower_nested_body_to_stmts(&if_stmt.body, loop_ctx, next_label_id)?;
            let orelse =
                lower_orelse_to_stmts(&if_stmt.elif_else_clauses, stmt, loop_ctx, next_label_id)?;
            out.push(BlockPyStmt::If(BlockPyIf {
                test: (*if_stmt.test).clone().into(),
                body,
                orelse,
            }));
        }
        Stmt::While(_) => {
            panic!("While should be lowered before Ruff AST -> BlockPy stmt-list conversion");
        }
        Stmt::For(_) => {
            panic!("For should be lowered before Ruff AST -> BlockPy stmt-list conversion");
        }
        Stmt::With(with_stmt) => {
            lower_with_into(with_stmt.clone(), out, loop_ctx, next_label_id)?;
        }
        Stmt::Match(_) => {
            panic!("Match should be lowered before Ruff AST -> BlockPy conversion");
        }
        Stmt::Assert(_) => {
            panic!("Assert should be lowered before Ruff AST -> BlockPy conversion");
        }
        Stmt::Import(_) => {
            panic!("Import should be lowered before Ruff AST -> BlockPy conversion");
        }
        Stmt::ImportFrom(_) => {
            panic!("ImportFrom should be lowered before Ruff AST -> BlockPy conversion");
        }
        Stmt::Break(_) => {
            if let Some(loop_ctx) = loop_ctx {
                out.push(BlockPyStmt::Jump(loop_ctx.break_label.clone()));
            } else {
                panic!("Break should be lowered before Ruff AST -> BlockPy conversion");
            }
        }
        Stmt::Continue(_) => {
            if let Some(loop_ctx) = loop_ctx {
                out.push(BlockPyStmt::Jump(loop_ctx.continue_label.clone()));
            } else {
                panic!("Continue should be lowered before Ruff AST -> BlockPy conversion");
            }
        }
        Stmt::Return(return_stmt) => {
            out.push(BlockPyStmt::Return(
                return_stmt.value.as_ref().map(|v| (**v).clone().into()),
            ));
        }
        Stmt::Raise(raise_stmt) => {
            if raise_stmt.cause.is_some() {
                panic!("raise-from should be lowered before Ruff AST -> BlockPy conversion");
            }
            out.push(BlockPyStmt::Raise(BlockPyRaise {
                exc: raise_stmt.exc.as_ref().map(|exc| (**exc).clone().into()),
            }));
        }
        Stmt::Try(_) => {
            panic!("Try should be lowered through stmt-sequence BlockPy conversion");
        }
        other => {
            return Err(format!(
                "unsupported statement reached Ruff AST -> BlockPy conversion: {}\nstmt:\n{}",
                stmt_kind_name(other),
                ruff_ast_to_string(other).trim_end()
            ));
        }
    }
    Ok(())
}

fn maybe_placeholder(expr: Expr) -> (Stmt, Expr, bool) {
    if is_simple(&expr) && !matches!(&expr, Expr::StringLiteral(_) | Expr::BytesLiteral(_)) {
        return (empty_body().into(), expr, false);
    }
    let tmp = fresh_name("tmp");
    let stmt = py_stmt!("{tmp:id} = {expr:expr}", tmp = tmp.as_str(), expr = expr);
    (stmt, py_expr!("{tmp:id}", tmp = tmp.as_str()), true)
}

fn with_target_object_expr(value: Expr) -> Expr {
    if let Expr::Name(name) = &value {
        py_expr!(
            "__dp_load_deleted_name({name:literal}, {value:expr})",
            name = name.id.as_str(),
            value = value,
        )
    } else {
        value
    }
}

pub(crate) fn lower_with_stmt_sequence<F>(
    with_stmt: ast::StmtWith,
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    jump_label: Option<String>,
    lower_sequence: &mut F,
) -> String
where
    F: FnMut(&[Box<Stmt>], String, &mut Vec<BlockPyBlock>) -> String,
{
    let lowered_with = desugar_with_stmt_for_bb(with_stmt);
    lower_expanded_stmt_sequence(
        lowered_with,
        remaining_stmts,
        cont_label,
        linear,
        blocks,
        jump_label,
        lower_sequence,
    )
}

pub(crate) fn lower_star_try_stmt_sequence<F>(
    try_stmt: ast::StmtTry,
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    jump_label: Option<String>,
    lower_sequence: &mut F,
) -> String
where
    F: FnMut(&[Box<Stmt>], String, &mut Vec<BlockPyBlock>) -> String,
{
    let rewritten_try = match rewrite_stmt::exception::rewrite_try(try_stmt) {
        Rewrite::Walk(stmt) | Rewrite::Unmodified(stmt) => stmt,
    };
    lower_expanded_stmt_sequence(
        rewritten_try,
        remaining_stmts,
        cont_label,
        linear,
        blocks,
        jump_label,
        lower_sequence,
    )
}

pub(crate) fn lower_try_stmt_sequence<F>(
    try_stmt: ast::StmtTry,
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    try_plan: TryPlan,
    lower_sequence: &mut F,
) -> (String, TryRegionPlan)
where
    F: FnMut(&[Box<Stmt>], String, &mut Vec<BlockPyBlock>) -> String,
{
    let rest_entry = lower_sequence(remaining_stmts, cont_label.clone(), blocks);

    let else_body = flatten_stmt_boxes(&try_stmt.orelse.body);
    let try_body = flatten_stmt_boxes(&try_stmt.body.body);
    let except_body =
        (!try_stmt.handlers.is_empty()).then(|| prepare_except_body(&try_stmt.handlers));
    let finally_body = if !try_stmt.finalbody.body.is_empty() {
        Some(prepare_finally_body(
            &try_stmt.finalbody,
            try_plan.finally_exc_name.as_deref(),
        ))
    } else {
        None
    };

    let lowered_try = lower_try_regions(
        blocks,
        &try_plan,
        rest_entry.as_str(),
        finally_body,
        else_body,
        try_body,
        except_body,
        lower_sequence,
    );

    finalize_try_regions(
        blocks,
        label,
        linear,
        lowered_try.body_label,
        lowered_try.except_label,
        try_plan,
        lowered_try.body_region_range,
        lowered_try.else_region_range,
        lowered_try.except_region_range,
        lowered_try.finally_region_range,
        lowered_try.finally_label,
        lowered_try.finally_normal_entry,
    )
}

fn rewrite_assignment_target<F>(target: Expr, rhs: Expr, out: &mut Vec<Stmt>, next_temp: &mut F)
where
    F: FnMut(&str) -> String,
{
    match target {
        Expr::Tuple(tuple) => rewrite_unpack_target(tuple.elts, rhs, out, next_temp),
        Expr::List(list) => rewrite_unpack_target(list.elts, rhs, out, next_temp),
        Expr::Subscript(ast::ExprSubscript { value, slice, .. }) => {
            out.push(py_stmt!(
                "__dp_setitem({obj:expr}, {key:expr}, {rhs:expr})",
                obj = with_target_object_expr(*value),
                key = *slice,
                rhs = rhs,
            ));
        }
        Expr::Attribute(ast::ExprAttribute { value, attr, .. }) => {
            out.push(py_stmt!(
                "__dp_setattr({obj:expr}, {name:literal}, {rhs:expr})",
                obj = with_target_object_expr(*value),
                name = attr.as_str(),
                rhs = rhs,
            ));
        }
        Expr::Name(ast::ExprName { id, .. }) => {
            out.push(py_stmt!(
                "{name:id} = {rhs:expr}",
                name = id.as_str(),
                rhs = rhs
            ));
        }
        other => {
            panic!("unsupported assignment target in Ruff AST -> BlockPy lowering: {other:?}");
        }
    }
}

fn rewrite_unpack_target<F>(elts: Vec<Expr>, value: Expr, out: &mut Vec<Stmt>, next_temp: &mut F)
where
    F: FnMut(&str) -> String,
{
    let unpacked_name = next_temp("tmp");
    let unpacked_tmp = py_expr!("{tmp:id}", tmp = unpacked_name.as_str());

    let mut spec_elts = Vec::new();
    let mut starred_seen = false;
    for elt in &elts {
        match elt {
            Expr::Starred(_) => {
                if starred_seen {
                    panic!("unsupported starred with-target assignment");
                }
                starred_seen = true;
                spec_elts.push(py_expr!("False"));
            }
            _ => spec_elts.push(py_expr!("True")),
        }
    }

    out.push(py_stmt!(
        "{tmp:id} = __dp_unpack({value:expr}, {spec:expr})",
        tmp = unpacked_name.as_str(),
        value = value,
        spec = make_tuple(spec_elts),
    ));

    let starred_index = elts.iter().position(|elt| matches!(elt, Expr::Starred(_)));
    for (idx, elt) in elts.into_iter().enumerate() {
        match elt {
            Expr::Starred(ast::ExprStarred { value, .. }) if Some(idx) == starred_index => {
                rewrite_assignment_target(
                    *value,
                    py_expr!(
                        "__dp_list(__dp_getitem({tmp:expr}, {idx:literal}))",
                        tmp = unpacked_tmp.clone(),
                        idx = idx as i64,
                    ),
                    out,
                    next_temp,
                );
            }
            other => {
                rewrite_assignment_target(
                    other,
                    py_expr!(
                        "__dp_getitem({tmp:expr}, {idx:literal})",
                        tmp = unpacked_tmp.clone(),
                        idx = idx as i64,
                    ),
                    out,
                    next_temp,
                );
            }
        }
    }

    out.push(py_stmt!("del {tmp:id}", tmp = unpacked_name.as_str()));
}

fn lower_generated_stmts_into_blockpy(
    stmts: Vec<Stmt>,
    out: &mut Vec<BlockPyStmt>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<(), String> {
    for stmt in lower_coroutine_awaits_in_stmts(stmts) {
        lower_stmt_into(&stmt, out, loop_ctx, next_label_id)?;
    }
    Ok(())
}

fn assignment_target_body(
    target: Expr,
    rhs: Expr,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<Vec<BlockPyStmt>, String> {
    let mut stmts = Vec::new();
    let mut next_temp = |prefix: &str| fresh_name(prefix);
    rewrite_assignment_target(target, rhs, &mut stmts, &mut next_temp);
    let mut out = Vec::new();
    lower_generated_stmts_into_blockpy(stmts, &mut out, loop_ctx, next_label_id)?;
    Ok(out)
}

pub(crate) fn build_for_target_assign_body<F>(
    target: &Expr,
    tmp_expr: Expr,
    tmp_name: &str,
    cell_slots: &std::collections::HashSet<String>,
    next_temp: &mut F,
) -> Vec<Stmt>
where
    F: FnMut(&str) -> String,
{
    let mut out = Vec::new();
    rewrite_assignment_target(target.clone(), tmp_expr, &mut out, next_temp);
    out.extend(sync_target_cells_stmts_shared(target, cell_slots));
    out.push(py_stmt!("{tmp:id} = None", tmp = tmp_name));
    out
}

pub(crate) fn desugar_with_stmt_for_bb(with_stmt: ast::StmtWith) -> Stmt {
    if with_stmt.items.is_empty() {
        return Stmt::BodyStmt(with_stmt.body);
    }

    let ast::StmtWith {
        items,
        body,
        is_async,
        ..
    } = with_stmt;

    let mut lowered_body: Stmt = body.into();

    for ast::WithItem {
        context_expr,
        optional_vars,
        ..
    } in items.into_iter().rev()
    {
        let target = optional_vars.map(|var| *var);
        let exit_name = fresh_name("with_exit");
        let ok_name = fresh_name("with_ok");
        let suppress_name = fresh_name("with_suppress");
        let (ctx_placeholder_stmt, ctx_expr, ctx_was_placeholder) = maybe_placeholder(context_expr);
        let ctx_cleanup = if ctx_was_placeholder {
            py_stmt!("{ctx:expr} = None", ctx = ctx_expr.clone())
        } else {
            empty_body().into()
        };

        // Transitional desugaring: keep with semantics stable while Ruff AST ->
        // BlockPy owns this lowering. The long-term goal is a more direct BlockPy
        // representation that does not need the ok/suppress bookkeeping temps.
        let enter_value = if is_async {
            py_expr!(
                "await __dp_asynccontextmanager_aenter({ctx:expr})",
                ctx = ctx_expr.clone()
            )
        } else {
            py_expr!(
                "__dp_contextmanager_enter({ctx:expr})",
                ctx = ctx_expr.clone()
            )
        };
        let enter_stmt = if let Some(target) = target.clone() {
            let mut enter_stmts = Vec::new();
            let mut next_temp = |prefix: &str| fresh_name(prefix);
            rewrite_assignment_target(target, enter_value, &mut enter_stmts, &mut next_temp);
            into_body(enter_stmts)
        } else {
            py_stmt!("{value:expr}", value = enter_value)
        };

        lowered_body = if is_async {
            py_stmt!(
                r#"
{ctx_placeholder_stmt:stmt}
{exit_name:id} = __dp_asynccontextmanager_get_aexit({ctx_expr:expr})
{enter_stmt:stmt}
{ok_name:id} = True
try:
    {body:stmt}
except BaseException:
    {ok_name:id} = False
    {suppress_name:id} = await __dp_asynccontextmanager_aexit({exit_name:id}, __dp_exc_info())
    if not {suppress_name:id}:
        raise
finally:
    if {ok_name:id}:
        await __dp_asynccontextmanager_aexit({exit_name:id}, None)
    {exit_name:id} = None
    {ctx_cleanup:stmt}
"#,
                ctx_placeholder_stmt = ctx_placeholder_stmt,
                ctx_expr = ctx_expr.clone(),
                enter_stmt = enter_stmt,
                body = lowered_body,
                exit_name = exit_name.as_str(),
                ok_name = ok_name.as_str(),
                suppress_name = suppress_name.as_str(),
                ctx_cleanup = ctx_cleanup,
            )
        } else {
            py_stmt!(
                r#"
{ctx_placeholder_stmt:stmt}
{exit_name:id} = __dp_contextmanager_get_exit({ctx_expr:expr})
{enter_stmt:stmt}
{ok_name:id} = True
try:
    {body:stmt}
except BaseException:
    {ok_name:id} = False
    __dp_contextmanager_exit({exit_name:id}, __dp_exc_info())
finally:
    if {ok_name:id}:
        __dp_contextmanager_exit({exit_name:id}, None)
    {exit_name:id} = None
    {ctx_cleanup:stmt}
"#,
                ctx_placeholder_stmt = ctx_placeholder_stmt,
                ctx_expr = ctx_expr.clone(),
                enter_stmt = enter_stmt,
                body = lowered_body,
                exit_name = exit_name.as_str(),
                ok_name = ok_name.as_str(),
                ctx_cleanup = ctx_cleanup,
            )
        };
    }

    if is_async {
        lower_coroutine_awaits_in_stmt(lowered_body)
    } else {
        lowered_body
    }
}

fn lower_with_into(
    with_stmt: ast::StmtWith,
    out: &mut Vec<BlockPyStmt>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<(), String> {
    let lowered_body = desugar_with_stmt_for_bb(with_stmt);
    lower_stmt_into(&lowered_body, out, loop_ctx, next_label_id)
}

fn lower_body_to_blocks_with_entry(
    body: &StmtBody,
    entry_label: BlockPyLabel,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
    fallthrough_target: Option<BlockPyLabel>,
) -> Result<Vec<BlockPyBlock>, String> {
    let mut blocks = Vec::new();
    let mut current_label = entry_label;
    let mut current_body = Vec::new();

    for stmt in &body.body {
        match stmt.as_ref() {
            Stmt::While(while_stmt) => {
                let test_label = fresh_blockpy_label("while_test", next_label_id);
                let body_label = fresh_blockpy_label("while_body", next_label_id);
                let after_label = fresh_blockpy_label("while_after", next_label_id);
                let else_label = if while_stmt.orelse.body.is_empty() {
                    None
                } else {
                    Some(fresh_blockpy_label("while_else", next_label_id))
                };

                blocks.push(finalize_blockpy_block(
                    current_label.clone(),
                    current_body,
                    Some(test_label.clone()),
                ));

                blocks.push(BlockPyBlock {
                    label: test_label.clone(),
                    exc_param: None,
                    body: Vec::new(),
                    term: BlockPyTerm::IfTerm(BlockPyIfTerm {
                        test: (*while_stmt.test).clone().into(),
                        body: Box::new(BlockPyBlock {
                            label: BlockPyLabel::from(format!("{}_then", test_label.as_str())),
                            exc_param: None,
                            body: Vec::new(),
                            term: BlockPyTerm::Jump(body_label.clone()),
                        }),
                        orelse: Box::new(BlockPyBlock {
                            label: BlockPyLabel::from(format!("{}_else", test_label.as_str())),
                            exc_param: None,
                            body: Vec::new(),
                            term: BlockPyTerm::Jump(
                                else_label.clone().unwrap_or_else(|| after_label.clone()),
                            ),
                        }),
                    }),
                });

                let inner_loop_ctx = LoopContext {
                    continue_label: test_label.clone(),
                    break_label: after_label.clone(),
                };
                let loop_body = lower_body_to_blocks_with_entry(
                    &while_stmt.body,
                    body_label.clone(),
                    Some(&inner_loop_ctx),
                    next_label_id,
                    Some(test_label.clone()),
                )?;
                blocks.extend(loop_body);

                if let Some(else_label) = else_label {
                    blocks.extend(lower_body_to_blocks_with_entry(
                        &while_stmt.orelse,
                        else_label,
                        loop_ctx,
                        next_label_id,
                        Some(after_label.clone()),
                    )?);
                }

                current_label = after_label;
                current_body = Vec::new();
            }
            Stmt::For(for_stmt) => {
                let setup_label = fresh_blockpy_label("for_setup", next_label_id);
                let fetch_label = fresh_blockpy_label("for_fetch", next_label_id);
                let assign_label = fresh_blockpy_label("for_assign", next_label_id);
                let body_label = fresh_blockpy_label("for_body", next_label_id);
                let after_label = fresh_blockpy_label("for_after", next_label_id);
                let else_label = if for_stmt.orelse.body.is_empty() {
                    None
                } else {
                    Some(fresh_blockpy_label("for_else", next_label_id))
                };
                let iter_name = fresh_name("iter");
                let target_tmp = fresh_name("tmp");

                blocks.push(finalize_blockpy_block(
                    current_label.clone(),
                    current_body,
                    Some(setup_label.clone()),
                ));

                let iter_expr = if for_stmt.is_async {
                    py_expr!("__dp_aiter({iter:expr})", iter = *for_stmt.iter.clone())
                } else {
                    py_expr!("__dp_iter({iter:expr})", iter = *for_stmt.iter.clone())
                };
                blocks.push(BlockPyBlock {
                    label: setup_label.clone(),
                    exc_param: None,
                    body: vec![BlockPyStmt::Assign(BlockPyAssign {
                        target: py_expr!("{name:id}", name = iter_name.as_str())
                            .as_name_expr()
                            .expect("fresh iter temp should be a name")
                            .clone(),
                        value: iter_expr.into(),
                    })],
                    term: BlockPyTerm::Jump(fetch_label.clone()),
                });

                let fetch_value = if for_stmt.is_async {
                    py_expr!(
                        "await __dp_anext_or_sentinel({iter:expr})",
                        iter = py_expr!("{name:id}", name = iter_name.as_str())
                    )
                } else {
                    py_expr!(
                        "__dp_next_or_sentinel({iter:expr})",
                        iter = py_expr!("{name:id}", name = iter_name.as_str())
                    )
                };

                let mut false_body = assignment_target_body(
                    *for_stmt.target.clone(),
                    py_expr!("{tmp:id}", tmp = target_tmp.as_str()),
                    None,
                    next_label_id,
                )?;
                false_body.push(BlockPyStmt::Assign(BlockPyAssign {
                    target: py_expr!("{tmp:id}", tmp = target_tmp.as_str())
                        .as_name_expr()
                        .expect("fresh target temp should be a name")
                        .clone(),
                    value: py_expr!("None").into(),
                }));

                blocks.push(BlockPyBlock {
                    label: fetch_label.clone(),
                    exc_param: None,
                    body: vec![BlockPyStmt::Assign(BlockPyAssign {
                        target: py_expr!("{tmp:id}", tmp = target_tmp.as_str())
                            .as_name_expr()
                            .expect("fresh target temp should be a name")
                            .clone(),
                        value: fetch_value.into(),
                    })],
                    term: BlockPyTerm::IfTerm(BlockPyIfTerm {
                        test: py_expr!(
                            "__dp_is_({value:expr}, __dp__.ITER_COMPLETE)",
                            value = py_expr!("{tmp:id}", tmp = target_tmp.as_str())
                        )
                        .into(),
                        body: Box::new(BlockPyBlock {
                            label: BlockPyLabel::from(format!("{}_then", fetch_label.as_str())),
                            exc_param: None,
                            body: Vec::new(),
                            term: BlockPyTerm::Jump(
                                else_label.clone().unwrap_or_else(|| after_label.clone()),
                            ),
                        }),
                        orelse: Box::new(BlockPyBlock {
                            label: BlockPyLabel::from(format!("{}_else", fetch_label.as_str())),
                            exc_param: None,
                            body: Vec::new(),
                            term: BlockPyTerm::Jump(assign_label.clone()),
                        }),
                    }),
                });

                blocks.push(finalize_blockpy_block(
                    assign_label,
                    false_body,
                    Some(body_label.clone()),
                ));

                let inner_loop_ctx = LoopContext {
                    continue_label: fetch_label.clone(),
                    break_label: after_label.clone(),
                };
                let loop_body = lower_body_to_blocks_with_entry(
                    &for_stmt.body,
                    body_label.clone(),
                    Some(&inner_loop_ctx),
                    next_label_id,
                    Some(fetch_label.clone()),
                )?;
                blocks.extend(loop_body);

                if let Some(else_label) = else_label {
                    blocks.extend(lower_body_to_blocks_with_entry(
                        &for_stmt.orelse,
                        else_label,
                        loop_ctx,
                        next_label_id,
                        Some(after_label.clone()),
                    )?);
                }

                current_label = after_label;
                current_body = Vec::new();
            }
            _ => lower_stmt_into(stmt.as_ref(), &mut current_body, loop_ctx, next_label_id)?,
        }
    }

    blocks.push(finalize_blockpy_block(
        current_label,
        current_body,
        fallthrough_target,
    ));
    Ok(blocks)
}

fn fresh_blockpy_label(prefix: &str, next_label_id: &mut usize) -> BlockPyLabel {
    let label = BlockPyLabel::from(format!("{prefix}_{next_label_id}"));
    *next_label_id += 1;
    label
}

fn lower_orelse_to_stmts(
    clauses: &[ast::ElifElseClause],
    stmt: &Stmt,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<Vec<BlockPyStmt>, String> {
    match clauses {
        [] => Ok(Vec::new()),
        [clause] if clause.test.is_none() => {
            lower_nested_body_to_stmts(&clause.body, loop_ctx, next_label_id)
        }
        _ => Err(format!(
            "`elif` chain reached Ruff AST -> BlockPy conversion\nstmt:\n{}",
            ruff_ast_to_string(stmt).trim_end()
        )),
    }
}

fn is_ignorable_module_stmt(stmt: &Stmt) -> bool {
    matches!(
        stmt,
        Stmt::Expr(ast::StmtExpr { value, .. }) if matches!(value.as_ref(), Expr::StringLiteral(_))
    ) || matches!(
        stmt,
        Stmt::ImportFrom(ast::StmtImportFrom { module, .. })
            if module.as_ref().map(|name| name.id.as_str()) == Some("__future__")
    ) || matches!(stmt, Stmt::Pass(_))
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
    use crate::basic_block::collect_function_identity_by_node;
    use crate::basic_block::ruff_to_blockpy::generator_lowering::build_closure_backed_generator_factory_block;

    fn wrapped_module(source: &str) -> (StmtBody, FunctionIdentityByNode) {
        let mut module = ruff_python_parser::parse_module(source)
            .unwrap()
            .into_syntax()
            .body;
        crate::driver::wrap_module_init(&mut module);
        let scope = crate::analyze_module_scope(&mut module);
        let identities = collect_function_identity_by_node(&mut module, scope);
        (module, identities)
    }

    fn wrapped_blockpy(source: &str) -> BlockPyModule {
        let (module, identities) = wrapped_module(source);
        rewrite_ast_to_blockpy_module(&module, &identities).unwrap()
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
            rendered.contains("function gen(n) [kind=generator"),
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
            inherited_captures: vec![crate::basic_block::bb_ir::BbClosureSlot {
                logical_name: "captured".to_string(),
                storage_name: "_dp_cell_captured".to_string(),
                init: crate::basic_block::bb_ir::BbClosureInit::InheritedCapture,
            }],
            lifted_locals: vec![crate::basic_block::bb_ir::BbClosureSlot {
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
    fn lower_with_stmt_sequence_desugars_before_recursing() {
        let module = ruff_python_parser::parse_module(
            r#"
def f(ctx):
    with ctx():
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
        let mut saw_expanded = false;
        let entry = lower_with_stmt_sequence(
            with_stmt.clone(),
            &[],
            "cont".to_string(),
            Vec::new(),
            &mut blocks,
            None,
            &mut |expanded, cont_label, _blocks| {
                saw_expanded = true;
                assert_eq!(cont_label, "cont");
                assert!(expanded
                    .iter()
                    .any(|stmt| matches!(stmt.as_ref(), Stmt::Try(_))));
                "expanded_entry".to_string()
            },
        );

        assert!(saw_expanded);
        assert_eq!(entry, "expanded_entry");
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
    fn requires_wrapped_module_input() {
        let module = ruff_python_parser::parse_module(
            r#"
x = 1

def f():
    return 1
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let err = rewrite_ast_to_blockpy_module(&module, &FunctionIdentityByNode::default())
            .expect_err("raw modules should be rejected");
        assert!(err.contains("module-init-wrapped input"), "{err}");
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
