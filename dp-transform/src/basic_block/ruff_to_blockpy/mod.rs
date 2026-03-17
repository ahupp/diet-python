use super::await_lower::{
    blockpy_blocks_contain_await_exprs, lower_coroutine_awaits_in_blockpy_blocks,
    lower_coroutine_awaits_to_yield_from,
};
use super::block_py::cfg::{
    fold_constant_brif_blockpy, fold_jumps_to_trivial_none_return_blockpy,
    prune_unreachable_blockpy_blocks, relabel_blockpy_blocks, rename_blockpy_labels,
};
use super::block_py::dataflow::compute_block_params_blockpy;
use super::block_py::exception::{
    contains_return_stmt_in_body, contains_return_stmt_in_handlers,
    rewrite_region_returns_to_finally_blockpy,
};
use super::block_py::state::{
    collect_parameter_names, collect_state_vars,
    sync_target_cells_stmts as sync_target_cells_stmts_shared,
};
use super::block_py::{
    assert_blockpy_block_normalized, BlockPyBlockMeta, BlockPyCallableFacts, BlockPyCallableHeader,
    BlockPyFunctionKind, BlockPyLabel, BlockPyTryJump, SemanticBlockPyBlock,
    SemanticBlockPyCallableDef, SemanticBlockPyTerm, ENTRY_BLOCK_LABEL,
};
use super::cfg_ir::CfgCallableDef;
use super::lowered_ir::{
    BindingTarget, BoundCallable, ClosureLayout, FunctionId, LoweredFunction, LoweredFunctionKind,
    LoweredRuntimeMetadata,
};
use super::stmt_utils::flatten_stmt_boxes;
use crate::basic_block::ast_to_ast::ast_rewrite::Rewrite;
use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::expr_utils::make_tuple;
use crate::namegen::fresh_name;
use crate::ruff_ast_to_string;
use crate::template::{empty_body, into_body, is_simple};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt, StmtBody};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
mod compat;
pub(crate) mod expr_lowering;
mod stmt_lowering;
mod stmt_sequences;
mod try_regions;

pub(crate) use super::blockpy_generators::{
    build_async_for_continue_entry, build_blockpy_closure_layout, build_initial_generator_metadata,
    compute_runtime_export_data, lower_closure_backed_generator_export_bundle,
    lower_generator_block_plan, prepare_generator_runtime_blocks_for_export,
    synthesize_generator_dispatch_metadata, try_lower_generators_from_await_free_semantic_input,
    GeneratorBlockPlan, GeneratorLoweringRoute, GeneratorMetadata, GeneratorYieldSite,
    PostBlockPyGeneratorLowering, RuntimeExportData, SemanticGeneratorInput,
};

pub(crate) use compat::{
    compat_block_from_blockpy, compat_if_jump_block, compat_jump_block_from_blockpy,
    compat_next_label, compat_next_temp, compat_raise_block_from_blockpy_raise,
    compat_return_block_from_expr, emit_for_loop_blocks, emit_if_branch_block_with_expr_setup,
    emit_sequence_jump_block, emit_sequence_raise_block_with_expr_setup,
    emit_sequence_return_block_with_expr_setup, emit_simple_while_blocks_with_expr_setup,
    lower_for_loop_continue_entry_with_state,
};
pub(crate) use stmt_lowering::{
    build_for_target_assign_body, lower_star_try_stmt_sequence, lower_stmt_into,
    lower_try_stmt_sequence, lower_with_stmt_sequence, rewrite_assign_stmt, rewrite_augassign_stmt,
    rewrite_delete_stmt, rewrite_type_alias_stmt,
};
pub(crate) use stmt_sequences::{
    lower_expanded_stmt_sequence, lower_stmt_sequence_with_state, lower_stmts_to_blockpy_stmts,
    lower_stmts_to_blockpy_stmts_with_context,
};
pub(crate) use try_regions::{
    block_references_label, build_try_plan, finalize_try_regions, lower_try_regions,
    prepare_except_body, prepare_finally_body, TryPlan,
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

#[derive(Debug, Clone)]
pub struct LoweredBlockPyBridgeMetadata {
    pub block_params: HashMap<String, Vec<String>>,
    pub exception_edges: HashMap<String, Option<String>>,
}

#[derive(Debug, Clone)]
pub struct LoweredBlockPyMetadata {
    pub runtime: LoweredRuntimeMetadata,
    pub bridge: LoweredBlockPyBridgeMetadata,
}

pub type LoweredBlockPyFunction<C = SemanticBlockPyCallableDef> =
    LoweredFunction<C, LoweredBlockPyMetadata>;

impl<C> LoweredFunction<C, LoweredBlockPyMetadata> {
    pub fn map_callable_def<D>(&self, f: impl FnOnce(&C) -> D) -> LoweredBlockPyFunction<D> {
        self.map_callable(f)
    }

    pub fn lowered_kind(&self) -> &LoweredFunctionKind {
        &self.extra.runtime.kind
    }

    pub fn block_params(&self) -> &HashMap<String, Vec<String>> {
        &self.extra.bridge.block_params
    }

    pub fn exception_edges(&self) -> &HashMap<String, Option<String>> {
        &self.extra.bridge.exception_edges
    }

    pub fn runtime_closure_layout(&self) -> &Option<ClosureLayout> {
        &self.extra.runtime.closure_layout
    }
}

pub(crate) struct LoweredBlockPyFunctionBundle {
    pub main_function: LoweredBlockPyFunction,
    pub helper_functions: Vec<LoweredBlockPyFunction>,
}

#[derive(Clone)]
pub(crate) struct LoweredBlockPyFunctionBundlePlan {
    pub prepared_function_plan: PreparedBlockPyFunctionPlan,
    pub has_yield: bool,
    pub is_coroutine: bool,
    pub is_async_generator_runtime: bool,
    pub is_closure_backed_generator_runtime: bool,
    pub extra_closure_state_names: Vec<String>,
    pub capture_names: Vec<String>,
    pub label_prefix: String,
    pub module_init_mode: bool,
}

#[derive(Clone)]
pub(crate) struct PreparedBlockPyFunction {
    pub callable_def: SemanticBlockPyCallableDef,
    pub generator_metadata: Option<GeneratorMetadata>,
    pub try_regions: Vec<TryRegionPlan>,
}

#[derive(Clone)]
pub(crate) enum PreparedBlockPyFunctionPlan {
    Ready(PreparedBlockPyFunction),
    PendingGeneratorLowering(PendingGeneratorLoweringPlan),
}

impl PreparedBlockPyFunctionPlan {
    pub(crate) fn callable_facts(&self) -> &BlockPyCallableFacts {
        match self {
            PreparedBlockPyFunctionPlan::Ready(prepared) => &prepared.callable_def.facts,
            PreparedBlockPyFunctionPlan::PendingGeneratorLowering(pending) => {
                &pending.callable_facts
            }
        }
    }

    pub(crate) fn callable_header(&self) -> BlockPyCallableHeader<ast::Parameters> {
        match self {
            PreparedBlockPyFunctionPlan::Ready(prepared) => prepared.callable_def.header(),
            PreparedBlockPyFunctionPlan::PendingGeneratorLowering(pending) => {
                pending.header.clone()
            }
        }
    }
}

impl LoweredBlockPyFunctionBundlePlan {
    pub(crate) fn callable_facts(&self) -> &BlockPyCallableFacts {
        self.prepared_function_plan.callable_facts()
    }

    pub(crate) fn callable_header(&self) -> BlockPyCallableHeader<ast::Parameters> {
        self.prepared_function_plan.callable_header()
    }

    pub(crate) fn param_names(&self) -> Vec<String> {
        collect_parameter_names(&self.callable_header().params)
    }
}

#[derive(Clone)]
pub(crate) struct PendingGeneratorLoweringPlan {
    pub header: BlockPyCallableHeader<ast::Parameters>,
    pub doc: Option<Expr>,
    pub semantic_input: SemanticGeneratorInput,
    pub end_label: String,
    pub blockpy_kind: BlockPyFunctionKind,
    pub callable_facts: BlockPyCallableFacts,
    pub awaits_remain_after_lowering: Option<bool>,
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
    Expanded(Stmt),
    FunctionDef(ast::StmtFunctionDef),
    Generator {
        plan: GeneratorBlockPlan,
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
) -> LoweredFunctionKind {
    match kind {
        BlockPyFunctionKind::Function | BlockPyFunctionKind::Coroutine => {
            LoweredFunctionKind::Function
        }
        BlockPyFunctionKind::Generator => LoweredFunctionKind::Generator {
            closure_state,
            resume_label: resume_label.to_string(),
            target_labels: target_labels.to_vec(),
            resume_pcs: resume_pcs.to_vec(),
        },
        BlockPyFunctionKind::AsyncGenerator => LoweredFunctionKind::AsyncGenerator {
            closure_state,
            resume_label: resume_label.to_string(),
            target_labels: target_labels.to_vec(),
            resume_pcs: resume_pcs.to_vec(),
        },
    }
}

pub(crate) fn build_blockpy_function(
    header: BlockPyCallableHeader<ast::Parameters>,
    doc: Option<Expr>,
    kind: BlockPyFunctionKind,
    entry_label: String,
    entry_liveins: Vec<String>,
    closure_layout: Option<ClosureLayout>,
    facts: BlockPyCallableFacts,
    local_cell_slots: Vec<String>,
    mut blocks: Vec<SemanticBlockPyBlock>,
) -> SemanticBlockPyCallableDef {
    move_blockpy_entry_block_to_front(&mut blocks, entry_label.as_str());
    for block in &blocks {
        assert_blockpy_block_normalized(block);
    }
    SemanticBlockPyCallableDef {
        cfg: CfgCallableDef {
            function_id: header.function_id,
            bind_name: header.bind_name,
            display_name: header.display_name,
            qualname: header.qualname,
            kind,
            params: header.params,
            entry_liveins,
            blocks,
        },
        fn_name: header.fn_name,
        doc,
        closure_layout,
        facts,
        local_cell_slots,
    }
}

fn move_blockpy_entry_block_to_front(blocks: &mut Vec<SemanticBlockPyBlock>, entry_label: &str) {
    if let Some(entry_index) = blocks
        .iter()
        .position(|block| block.label.as_str() == entry_label)
    {
        if entry_index != 0 {
            let entry_block = blocks.remove(entry_index);
            blocks.insert(0, entry_block);
        }
    }
}

pub(crate) fn take_next_function_id(next_function_id: &mut usize) -> FunctionId {
    let id = FunctionId(*next_function_id);
    *next_function_id += 1;
    id
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_lowered_blockpy_function(
    callable_def: SemanticBlockPyCallableDef,
    lowered_kind: LoweredFunctionKind,
    block_params: HashMap<String, Vec<String>>,
    exception_edges: HashMap<String, Option<String>>,
    runtime_closure_layout: Option<ClosureLayout>,
) -> LoweredBlockPyFunction {
    LoweredFunction {
        callable_def: BoundCallable {
            callable: callable_def,
            binding_target: BindingTarget::Local,
        },
        extra: LoweredBlockPyMetadata {
            runtime: LoweredRuntimeMetadata {
                kind: lowered_kind,
                closure_layout: runtime_closure_layout,
            },
            bridge: LoweredBlockPyBridgeMetadata {
                block_params,
                exception_edges,
            },
        },
    }
}

fn fresh_normalized_entry_collision_label(
    blocks: &[SemanticBlockPyBlock],
    rename: &HashMap<String, String>,
) -> String {
    let existing = blocks
        .iter()
        .map(|block| block.label.as_str().to_string())
        .collect::<HashSet<_>>();
    let mut next_id = 0usize;
    loop {
        let candidate = format!("{ENTRY_BLOCK_LABEL}_{next_id}");
        if !existing.contains(candidate.as_str())
            && !rename.values().any(|value| value == &candidate)
        {
            return candidate;
        }
        next_id += 1;
    }
}

fn rename_label_map_keys<T: Clone>(
    map: &HashMap<String, T>,
    rename: &HashMap<String, String>,
) -> HashMap<String, T> {
    map.iter()
        .map(|(label, value)| {
            (
                rename
                    .get(label.as_str())
                    .cloned()
                    .unwrap_or_else(|| label.clone()),
                value.clone(),
            )
        })
        .collect()
}

fn rename_exception_edges(
    edges: &HashMap<String, Option<String>>,
    rename: &HashMap<String, String>,
) -> HashMap<String, Option<String>> {
    edges
        .iter()
        .map(|(label, target)| {
            let rewritten_label = rename
                .get(label.as_str())
                .cloned()
                .unwrap_or_else(|| label.clone());
            let rewritten_target = target.as_ref().map(|name| {
                rename
                    .get(name.as_str())
                    .cloned()
                    .unwrap_or_else(|| name.clone())
            });
            (rewritten_label, rewritten_target)
        })
        .collect()
}

fn rename_lowered_function_kind(kind: &mut LoweredFunctionKind, rename: &HashMap<String, String>) {
    match kind {
        LoweredFunctionKind::Function => {}
        LoweredFunctionKind::Generator {
            resume_label,
            target_labels,
            resume_pcs,
            ..
        }
        | LoweredFunctionKind::AsyncGenerator {
            resume_label,
            target_labels,
            resume_pcs,
            ..
        } => {
            if let Some(rewritten) = rename.get(resume_label.as_str()) {
                *resume_label = rewritten.clone();
            }
            for label in target_labels.iter_mut() {
                if let Some(rewritten) = rename.get(label.as_str()) {
                    *label = rewritten.clone();
                }
            }
            for (label, _) in resume_pcs.iter_mut() {
                if let Some(rewritten) = rename.get(label.as_str()) {
                    *label = rewritten.clone();
                }
            }
        }
    }
}

pub(crate) fn normalize_exported_entry_block(
    entry_label: String,
    mut blocks: Vec<SemanticBlockPyBlock>,
    block_params: HashMap<String, Vec<String>>,
    exception_edges: HashMap<String, Option<String>>,
    mut bb_kind: LoweredFunctionKind,
) -> (
    Vec<SemanticBlockPyBlock>,
    HashMap<String, Vec<String>>,
    HashMap<String, Option<String>>,
    LoweredFunctionKind,
) {
    let mut rename = HashMap::new();
    if entry_label != ENTRY_BLOCK_LABEL {
        if blocks
            .iter()
            .any(|block| block.label.as_str() == ENTRY_BLOCK_LABEL)
        {
            rename.insert(
                ENTRY_BLOCK_LABEL.to_string(),
                fresh_normalized_entry_collision_label(&blocks, &rename),
            );
        }
        rename.insert(entry_label.clone(), ENTRY_BLOCK_LABEL.to_string());
    }

    if !rename.is_empty() {
        rename_blockpy_labels(&rename, &mut blocks);
        rename_lowered_function_kind(&mut bb_kind, &rename);
    }

    if let Some(entry_index) = blocks
        .iter()
        .position(|block| block.label.as_str() == ENTRY_BLOCK_LABEL)
    {
        if entry_index != 0 {
            let entry_block = blocks.remove(entry_index);
            blocks.insert(0, entry_block);
        }
    }

    (
        blocks,
        rename_label_map_keys(&block_params, &rename),
        rename_exception_edges(&exception_edges, &rename),
        bb_kind,
    )
}

fn build_semantic_blockpy_closure_layout(
    param_names: &[String],
    entry_liveins: &[String],
    capture_names: &[String],
    local_cell_slots: &[String],
    injected_exception_names: &HashSet<String>,
) -> Option<ClosureLayout> {
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
    context: &Context,
    plan: LoweredBlockPyFunctionBundlePlan,
    next_block_id: &mut usize,
    next_function_id: &mut usize,
    lower_non_bb_def: &mut impl FnMut(&ast::StmtFunctionDef) -> Vec<Stmt>,
    next_temp: &mut impl FnMut(&str, &mut usize) -> String,
    prepare_callable_def: &mut impl FnMut(&mut SemanticBlockPyCallableDef),
) -> LoweredBlockPyFunctionBundle {
    let callable_facts = plan.callable_facts().clone();
    let callable_header = plan.callable_header();
    let param_names = plan.param_names();
    let LoweredBlockPyFunctionBundlePlan {
        prepared_function_plan,
        has_yield,
        is_coroutine,
        is_async_generator_runtime,
        is_closure_backed_generator_runtime,
        extra_closure_state_names,
        capture_names,
        label_prefix,
        module_init_mode,
    } = plan;
    let mut prepared_function = lower_generators_in_prepared_blockpy_function_plan(
        context,
        prepared_function_plan,
        is_async_generator_runtime,
        is_closure_backed_generator_runtime,
        label_prefix.as_str(),
        next_block_id,
        lower_non_bb_def,
        next_temp,
    );
    prepare_callable_def(&mut prepared_function.callable_def);
    let resume_function_id = has_yield.then(|| take_next_function_id(next_function_id));
    let mut cell_slots = callable_facts.cell_slots.clone();
    let PreparedBlockPyFunction {
        callable_def: mut blockpy_function,
        generator_metadata,
        try_regions,
    } = prepared_function;
    let entry_label = blockpy_function.entry_label().to_string();
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
        prepare_generator_runtime_blocks_for_export(
            &mut blocks_for_dataflow,
            &generator_yield_sites_for_lowering,
            is_async_generator_runtime,
            is_closure_backed_generator_runtime,
        );
    }

    let mut state_vars = collect_state_vars(&param_names, &blocks_for_dataflow, module_init_mode);
    for block in &blocks_for_dataflow {
        let Some(exc_param) = block.meta.exc_param.as_ref() else {
            continue;
        };
        if !state_vars.iter().any(|existing| existing == exc_param) {
            state_vars.push(exc_param.clone());
        }
    }
    if is_closure_backed_generator_runtime {
        for name in extra_closure_state_names {
            if !state_vars.iter().any(|existing| existing == &name) {
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
    let mut block_params =
        compute_block_params_blockpy(&blocks_for_dataflow, &state_vars, &extra_successors);
    for block in &blocks_for_dataflow {
        let Some(exc_param) = block.meta.exc_param.as_ref() else {
            continue;
        };
        let params = block_params
            .entry(block.label.as_str().to_string())
            .or_default();
        if !params.iter().any(|existing| existing == exc_param) {
            params.push(exc_param.clone());
        }
    }
    let RuntimeExportData {
        injected_exception_names,
        entry_liveins,
        closure_layout,
    } = compute_runtime_export_data(
        has_yield,
        &mut blocks_for_dataflow,
        &mut block_params,
        &param_names,
        &capture_names,
        &state_vars,
        generator_metadata.as_ref(),
        entry_label.as_str(),
        &mut cell_slots,
        &resume_pcs,
        &generator_yield_sites_for_lowering,
        is_async_generator_runtime,
        is_closure_backed_generator_runtime,
    );
    let extra_state_vars = if has_yield {
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

    let mut state_order = entry_liveins.clone();
    for name in extra_state_vars {
        if !state_order.iter().any(|existing| existing == &name) {
            state_order.push(name);
        }
    }

    let mut sorted_local_cell_slots = cell_slots.iter().cloned().collect::<Vec<_>>();
    sorted_local_cell_slots.sort();
    let semantic_closure_layout = if has_yield {
        closure_layout.clone()
    } else {
        build_semantic_blockpy_closure_layout(
            &param_names,
            &state_order,
            &capture_names,
            &sorted_local_cell_slots,
            &injected_exception_names,
        )
    };

    let function_id = blockpy_function.function_id;
    let bind_name = blockpy_function.bind_name.clone();
    let qualname = blockpy_function.qualname.clone();
    let doc = blockpy_function.doc.clone();
    let params = blockpy_function.params.clone();
    let display_name = callable_header.display_name;
    let runtime_entry_label = entry_label;
    let mut exported_entry_label = runtime_entry_label.clone();
    let mut exported_entry_liveins = state_order;
    let mut exported_blocks = blocks_for_dataflow;
    let mut exported_block_params = block_params;
    let mut exported_exception_edges = exception_edges;
    let mut helper_functions = Vec::new();

    if has_yield {
        let export = lower_closure_backed_generator_export_bundle(
            bind_name.as_str(),
            display_name.as_str(),
            qualname.as_str(),
            &params,
            &param_names,
            label_prefix.as_str(),
            runtime_entry_label.as_str(),
            exported_blocks,
            exported_block_params,
            exported_exception_edges,
            &closure_layout,
            &sorted_local_cell_slots,
            &target_labels,
            &resume_pcs,
            resume_function_id
                .expect("yielding export plan should preallocate a resume helper function id"),
            is_coroutine,
            is_async_generator_runtime,
        );
        helper_functions.extend(export.helper_functions);
        exported_blocks = export.exported_blocks;
        exported_entry_label = export.exported_entry_label;
        exported_entry_liveins = export.exported_entry_liveins;
        exported_block_params = export.exported_block_params;
        exported_exception_edges = export.exported_exception_edges;
    }

    let main_blockpy_kind = BlockPyFunctionKind::Function;
    let main_bb_kind = bb_kind_for_blockpy_kind(
        main_blockpy_kind,
        is_closure_backed_generator_runtime,
        runtime_entry_label.as_str(),
        &target_labels,
        &resume_pcs,
    );
    let (
        normalized_main_blocks,
        normalized_main_block_params,
        normalized_main_exception_edges,
        normalized_main_bb_kind,
    ) = normalize_exported_entry_block(
        exported_entry_label,
        exported_blocks,
        exported_block_params,
        exported_exception_edges,
        main_bb_kind,
    );
    let main_function = build_blockpy_function(
        BlockPyCallableHeader {
            function_id,
            fn_name: callable_header.fn_name,
            bind_name,
            display_name,
            qualname,
            params,
        },
        doc,
        main_blockpy_kind,
        ENTRY_BLOCK_LABEL.to_string(),
        exported_entry_liveins,
        if has_yield {
            None
        } else {
            semantic_closure_layout
        },
        blockpy_function.facts.clone(),
        sorted_local_cell_slots,
        normalized_main_blocks,
    );
    LoweredBlockPyFunctionBundle {
        main_function: build_lowered_blockpy_function(
            main_function,
            normalized_main_bb_kind,
            normalized_main_block_params,
            normalized_main_exception_edges,
            if has_yield { None } else { closure_layout },
        ),
        helper_functions,
    }
}

pub(crate) fn build_finalized_blockpy_function(
    header: BlockPyCallableHeader<ast::Parameters>,
    doc: Option<Expr>,
    kind: BlockPyFunctionKind,
    blocks: Vec<SemanticBlockPyBlock>,
    try_regions: Vec<TryRegionPlan>,
    entry_label: String,
    end_label: String,
    label_prefix: &str,
    generator_metadata: Option<GeneratorMetadata>,
    is_async_generator_runtime: bool,
    is_closure_backed_generator_runtime: bool,
    uncaught_exc_name: String,
    facts: BlockPyCallableFacts,
) -> PreparedBlockPyFunction {
    let callable_def = build_blockpy_function(
        header,
        doc,
        kind,
        entry_label.clone(),
        Vec::new(),
        None,
        facts,
        Vec::new(),
        blocks,
    );
    finalize_blockpy_function(
        callable_def,
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
fn build_semantic_generator_input<FDef, FTemp>(
    context: &Context,
    fn_name: &str,
    runtime_input_body: &[Box<Stmt>],
    fallback_runtime_input_body: &[Box<Stmt>],
    end_label: &str,
    is_closure_backed_generator_runtime: bool,
    cell_slots: &HashSet<String>,
    next_block_id: &mut usize,
    lower_non_bb_def: &mut FDef,
    next_temp: &mut FTemp,
) -> SemanticGeneratorInput
where
    FDef: FnMut(&ast::StmtFunctionDef) -> Vec<Stmt>,
    FTemp: FnMut(&str, &mut usize) -> String,
{
    let mut blocks = Vec::new();
    let mut try_regions = Vec::new();
    let mut state = BlockPySequenceGeneratorState {
        enabled: false,
        closure_state: is_closure_backed_generator_runtime,
        resume_order: Vec::new(),
        yield_sites: Vec::new(),
    };
    let entry_label = lower_stmt_sequence_with_state(
        context,
        fn_name,
        runtime_input_body,
        end_label.to_string(),
        None,
        None,
        &mut blocks,
        cell_slots,
        &mut state,
        &mut try_regions,
        next_block_id,
        lower_non_bb_def,
        next_temp,
    );
    SemanticGeneratorInput {
        fallback_runtime_input_body: fallback_runtime_input_body.to_vec(),
        blocks,
        entry_label,
        try_regions,
        resume_order: state.resume_order,
        yield_sites: state.yield_sites,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_prepared_blockpy_function_from_runtime_input<FDef, FTemp>(
    context: &Context,
    header: BlockPyCallableHeader<ast::Parameters>,
    runtime_input_body: &[Box<Stmt>],
    doc: Option<Expr>,
    end_label: String,
    label_prefix: &str,
    blockpy_kind: BlockPyFunctionKind,
    has_yield: bool,
    is_async_generator_runtime: bool,
    is_closure_backed_generator_runtime: bool,
    facts: &BlockPyCallableFacts,
    next_block_id: &mut usize,
    lower_non_bb_def: &mut FDef,
    next_temp: &mut FTemp,
) -> PreparedBlockPyFunction
where
    FDef: FnMut(&ast::StmtFunctionDef) -> Vec<Stmt>,
    FTemp: FnMut(&str, &mut usize) -> String,
{
    let fn_name = header.fn_name.clone();
    let mut blocks = Vec::new();
    let mut try_regions = Vec::new();
    let mut generator_state = BlockPySequenceGeneratorState {
        enabled: has_yield,
        closure_state: is_closure_backed_generator_runtime,
        resume_order: Vec::new(),
        yield_sites: Vec::new(),
    };
    let entry_label = lower_stmt_sequence_with_state(
        context,
        fn_name.as_str(),
        runtime_input_body,
        end_label.clone(),
        None,
        None,
        &mut blocks,
        &facts.cell_slots,
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
        header,
        doc,
        blockpy_kind,
        blocks,
        try_regions,
        entry_label,
        end_label,
        label_prefix,
        generator_metadata,
        is_async_generator_runtime,
        is_closure_backed_generator_runtime,
        next_temp("uncaught_exc", next_block_id),
        facts.clone(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_function_body_to_blockpy_function<FDef, FTemp>(
    context: &Context,
    runtime_input_body: &[Box<Stmt>],
    header: BlockPyCallableHeader<ast::Parameters>,
    doc: Option<Expr>,
    legacy_async_runtime_input_body: Option<&[Box<Stmt>]>,
    end_label: String,
    label_prefix: &str,
    has_yield: bool,
    coroutine_via_generator: bool,
    is_async_generator_runtime: bool,
    is_closure_backed_generator_runtime: bool,
    callable_facts: &BlockPyCallableFacts,
    next_block_id: &mut usize,
    lower_non_bb_def: &mut FDef,
    next_temp: &mut FTemp,
) -> PreparedBlockPyFunctionPlan
where
    FDef: FnMut(&ast::StmtFunctionDef) -> Vec<Stmt>,
    FTemp: FnMut(&str, &mut usize) -> String,
{
    let needs_generator_lowering = has_yield || coroutine_via_generator;
    let blockpy_kind = blockpy_kind_for_lowered_runtime(
        is_async_generator_runtime,
        coroutine_via_generator,
        has_yield,
    );
    if needs_generator_lowering {
        let semantic_input = build_semantic_generator_input(
            context,
            header.fn_name.as_str(),
            runtime_input_body,
            legacy_async_runtime_input_body.unwrap_or(runtime_input_body),
            end_label.as_str(),
            is_closure_backed_generator_runtime,
            &callable_facts.cell_slots,
            next_block_id,
            lower_non_bb_def,
            next_temp,
        );
        return PreparedBlockPyFunctionPlan::PendingGeneratorLowering(
            PendingGeneratorLoweringPlan {
                header,
                doc,
                semantic_input,
                end_label,
                blockpy_kind,
                callable_facts: callable_facts.clone(),
                awaits_remain_after_lowering: None,
            },
        );
    }
    PreparedBlockPyFunctionPlan::Ready(build_prepared_blockpy_function_from_runtime_input(
        context,
        header,
        runtime_input_body,
        doc,
        end_label,
        label_prefix,
        blockpy_kind,
        has_yield,
        is_async_generator_runtime,
        is_closure_backed_generator_runtime,
        callable_facts,
        next_block_id,
        lower_non_bb_def,
        next_temp,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_generators_in_prepared_blockpy_function_plan<FDef, FTemp>(
    context: &Context,
    plan: PreparedBlockPyFunctionPlan,
    is_async_generator_runtime: bool,
    is_closure_backed_generator_runtime: bool,
    label_prefix: &str,
    next_block_id: &mut usize,
    lower_non_bb_def: &mut FDef,
    next_temp: &mut FTemp,
) -> PreparedBlockPyFunction
where
    FDef: FnMut(&ast::StmtFunctionDef) -> Vec<Stmt>,
    FTemp: FnMut(&str, &mut usize) -> String,
{
    match plan {
        PreparedBlockPyFunctionPlan::Ready(prepared) => prepared,
        PreparedBlockPyFunctionPlan::PendingGeneratorLowering(pending) => {
            let awaits_remain_after_lowering = pending
                .awaits_remain_after_lowering
                .expect("semantic BlockPy await lowering should run before generator resolution");
            assert!(
                !awaits_remain_after_lowering,
                "semantic BlockPy await lowering should eliminate awaits before generator resolution",
            );
            if let Some(PostBlockPyGeneratorLowering {
                blocks,
                entry_label,
                try_regions,
                generator_metadata,
            }) = try_lower_generators_from_await_free_semantic_input(
                context,
                pending.semantic_input.clone(),
                GeneratorLoweringRoute {
                    is_closure_backed_generator_runtime,
                    fn_name: pending.header.fn_name.as_str(),
                    cell_slots: &pending.callable_facts.cell_slots,
                },
                next_block_id,
            )
            .expect("post-BlockPy generator lowering should not fail")
            {
                let header = pending.header.clone();
                return build_finalized_blockpy_function(
                    header,
                    pending.doc,
                    pending.blockpy_kind,
                    blocks,
                    try_regions,
                    entry_label,
                    pending.end_label,
                    label_prefix,
                    Some(generator_metadata),
                    is_async_generator_runtime,
                    is_closure_backed_generator_runtime,
                    next_temp("uncaught_exc", next_block_id),
                    pending.callable_facts,
                );
            }
            let mut fallback_runtime_input_body =
                pending.semantic_input.fallback_runtime_input_body.clone();
            lower_coroutine_awaits_to_yield_from(&mut fallback_runtime_input_body);
            let header = pending.header.clone();
            build_prepared_blockpy_function_from_runtime_input(
                context,
                header,
                &fallback_runtime_input_body,
                pending.doc,
                pending.end_label,
                label_prefix,
                pending.blockpy_kind,
                true,
                is_async_generator_runtime,
                is_closure_backed_generator_runtime,
                &pending.callable_facts,
                next_block_id,
                lower_non_bb_def,
                next_temp,
            )
        }
    }
}

fn lower_awaits_in_prepared_blockpy_function_plan(
    context: &Context,
    plan: PreparedBlockPyFunctionPlan,
) -> PreparedBlockPyFunctionPlan {
    match plan {
        PreparedBlockPyFunctionPlan::Ready(prepared) => {
            PreparedBlockPyFunctionPlan::Ready(prepared)
        }
        PreparedBlockPyFunctionPlan::PendingGeneratorLowering(mut pending) => {
            let await_lowered_blocks =
                lower_coroutine_awaits_in_blockpy_blocks(context, pending.semantic_input.blocks)
                    .expect("semantic BlockPy await lowering should not fail");
            let awaits_remain_after_lowering =
                blockpy_blocks_contain_await_exprs(&await_lowered_blocks);
            pending.semantic_input.blocks = await_lowered_blocks;
            pending.awaits_remain_after_lowering = Some(awaits_remain_after_lowering);
            PreparedBlockPyFunctionPlan::PendingGeneratorLowering(pending)
        }
    }
}

pub(crate) fn lower_awaits_in_lowered_blockpy_function_bundle_plan(
    context: &Context,
    mut plan: LoweredBlockPyFunctionBundlePlan,
) -> LoweredBlockPyFunctionBundlePlan {
    plan.prepared_function_plan =
        lower_awaits_in_prepared_blockpy_function_plan(context, plan.prepared_function_plan);
    plan
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
    blocks: &[SemanticBlockPyBlock],
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
    mut callable_def: SemanticBlockPyCallableDef,
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
        || callable_def
            .blocks
            .iter()
            .any(|block| block_references_label(block, end_label.as_str()));
    if needs_end_block {
        callable_def.blocks.push(SemanticBlockPyBlock {
            label: BlockPyLabel::from(end_label),
            body: Vec::new(),
            term: SemanticBlockPyTerm::Return(None),
            meta: BlockPyBlockMeta::default(),
        });
    }
    fold_jumps_to_trivial_none_return_blockpy(&mut callable_def.blocks);
    fold_constant_brif_blockpy(&mut callable_def.blocks);
    let prune_roots = generator_metadata
        .as_ref()
        .map(|info| info.resume_order.clone())
        .unwrap_or_default();
    prune_unreachable_blockpy_blocks(entry_label.as_str(), &prune_roots, &mut callable_def.blocks);
    let (relabelled_entry_label, label_rename) =
        relabel_blockpy_blocks(label_prefix, entry_label.as_str(), &mut callable_def.blocks);
    entry_label = relabelled_entry_label;
    relabel_try_regions(&mut try_regions, &label_rename);
    if let Some(generator) = generator_metadata.as_mut() {
        relabel_generator_info(generator, &label_rename);
        let resume_order = generator.resume_order.clone();
        let yield_sites = generator.yield_sites.clone();
        *generator = synthesize_generator_dispatch_metadata(
            &mut callable_def.blocks,
            &mut entry_label,
            label_prefix,
            is_async_generator_runtime,
            is_closure_backed_generator_runtime,
            uncaught_exc_name,
            &resume_order,
            &yield_sites,
        );
    }
    move_blockpy_entry_block_to_front(&mut callable_def.blocks, entry_label.as_str());
    PreparedBlockPyFunction {
        callable_def,
        generator_metadata,
        try_regions,
    }
}

#[derive(Clone)]
pub(crate) struct LoopContext {
    continue_label: BlockPyLabel,
    break_label: BlockPyLabel,
}

fn assign_delete_error(message: &str, stmt: &Stmt) -> String {
    format!("{message}\nstmt:\n{}", ruff_ast_to_string(stmt).trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basic_block::ast_to_ast::{context::Context, Options};
    use crate::basic_block::block_py::{
        SemanticBlockPyCallableDef as BlockPyCallableDef, SemanticBlockPyModule as BlockPyModule,
        SemanticBlockPyRaise as BlockPyRaise, SemanticBlockPyStmt as BlockPyStmt,
        SemanticBlockPyTerm as BlockPyTerm,
    };
    use crate::basic_block::blockpy_generators::{
        build_closure_backed_generator_factory_block, lower_generator_block_plan,
    };
    use crate::basic_block::ruff_to_blockpy::stmt_sequences::{
        lower_for_stmt_sequence, lower_generator_stmt_sequence_head, lower_if_stmt_sequence,
        lower_if_stmt_sequence_from_stmt, lower_while_stmt_sequence,
        lower_while_stmt_sequence_from_stmt, plan_generator_stmt_head_block,
        plan_stmt_sequence_head,
    };
    use crate::basic_block::ruff_to_blockpy::try_regions::build_try_plan;
    use crate::transform_str_to_blockpy_with_options;

    fn wrapped_blockpy(source: &str) -> BlockPyModule {
        transform_str_to_blockpy_with_options(source, Options::for_test()).unwrap()
    }

    fn function_by_name<'a>(blockpy: &'a BlockPyModule, bind_name: &str) -> &'a BlockPyCallableDef {
        blockpy
            .callable_defs
            .iter()
            .find(|func| func.bind_name == bind_name)
            .unwrap_or_else(|| panic!("missing BlockPy function {bind_name}; got {blockpy:?}"))
    }

    fn lower_stmt_for_panic_test(stmt: &Stmt) {
        let context = Context::new(Options::for_test(), "");
        let mut out = crate::basic_block::block_py::BlockPyCfgFragmentBuilder::<
            BlockPyStmt,
            BlockPyTerm,
        >::new();
        let mut next_label_id = 0usize;
        let _ = lower_stmt_into(&context, stmt, &mut out, None, &mut next_label_id);
    }

    fn test_context() -> Context {
        Context::new(Options::for_test(), "")
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
        assert!(rendered.contains("generator gen(n):"), "{rendered}");
        assert!(rendered.contains("function gen(n):"), "{rendered}");
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
        let context = test_context();
        let plan = plan_generator_stmt_head_block(&context, &stmt)
            .expect("expected generator stmt sequence plan");
        assert!(plan.needs_rest_entry);
        let label = lower_generator_block_plan(
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

        let context = test_context();
        let needs_rest_entry = plan_generator_stmt_head_block(&context, &stmt)
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

        let context = test_context();
        let needs_rest_entry = plan_generator_stmt_head_block(&context, &stmt)
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
            plan_stmt_sequence_head(&test_context(), stmt, true),
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
            plan_stmt_sequence_head(&test_context(), stmt, true),
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
            plan_stmt_sequence_head(&test_context(), stmt, false),
            StmtSequenceHeadPlan::Return(_)
        ));
        assert!(matches!(
            plan_stmt_sequence_head(&test_context(), stmt, true),
            StmtSequenceHeadPlan::Return(_)
        ));
    }

    #[test]
    fn stmt_sequence_head_plan_simplifies_assert_to_if() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    assert cond, msg
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
            plan_stmt_sequence_head(&test_context(), stmt, false),
            StmtSequenceHeadPlan::If(_)
        ));
    }

    #[test]
    fn stmt_sequence_head_plan_simplifies_match_to_expanded_if_chain() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    match "aa":
        case str(slot):
            return slot
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = func.body.body[0].as_ref();

        let StmtSequenceHeadPlan::Expanded(Stmt::BodyStmt(body)) =
            plan_stmt_sequence_head(&test_context(), stmt, false)
        else {
            panic!("expected expanded match body");
        };
        assert!(matches!(body.body[0].as_ref(), Stmt::Assign(_)));
        assert!(body
            .body
            .iter()
            .any(|stmt| matches!(stmt.as_ref(), Stmt::If(_))));
    }

    #[test]
    fn stmt_sequence_head_plan_re_expands_builtin_match_if_head() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    match "aa":
        case str(slot):
            return slot
        case _:
            return None
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = func.body.body[0].as_ref();

        let StmtSequenceHeadPlan::Expanded(Stmt::BodyStmt(body)) =
            plan_stmt_sequence_head(&test_context(), stmt, false)
        else {
            panic!("expected expanded match body");
        };
        let match_if = body
            .body
            .iter()
            .find(|stmt| matches!(stmt.as_ref(), Stmt::If(_)))
            .expect("expected expanded match body to contain an if");

        assert!(
            matches!(
                plan_stmt_sequence_head(&test_context(), match_if.as_ref(), false),
                StmtSequenceHeadPlan::If(_)
            ),
            "{}",
            crate::ruff_ast_to_string(match_if.as_ref()).trim_end()
        );
    }

    #[test]
    fn blockpy_match_builtin_class_pattern_lowers_short_circuit_test_before_bb() {
        let blockpy = wrapped_blockpy(
            r#"
def f():
    match "aa":
        case str(slot):
            return slot
        case _:
            return None
"#,
        );
        let rendered = crate::basic_block::blockpy_module_to_string(&blockpy);
        assert!(
            !rendered.contains("and __dp_match_class_attr_exists"),
            "{rendered}"
        );
        assert!(
            !rendered.contains("and __dp_match_class_attr_value"),
            "{rendered}"
        );
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

        assert!(plan_generator_stmt_head_block(&test_context(), stmt).is_none());
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

        let context = test_context();
        let generator_plan = plan_generator_stmt_head_block(&context, stmt)
            .expect("expected generator stmt sequence plan");
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

        let context = test_context();
        let generator_plan = plan_generator_stmt_head_block(&context, stmt)
            .expect("expected generator stmt sequence plan");
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
        let layout = crate::basic_block::lowered_ir::ClosureLayout {
            freevars: vec![crate::basic_block::lowered_ir::ClosureSlot {
                logical_name: "captured".to_string(),
                storage_name: "_dp_cell_captured".to_string(),
                init: crate::basic_block::lowered_ir::ClosureInit::InheritedCapture,
            }],
            cellvars: vec![crate::basic_block::lowered_ir::ClosureSlot {
                logical_name: "x".to_string(),
                storage_name: "_dp_cell_x".to_string(),
                init: crate::basic_block::lowered_ir::ClosureInit::Parameter,
            }],
            runtime_cells: vec![crate::basic_block::lowered_ir::ClosureSlot {
                logical_name: "_dp_pc".to_string(),
                storage_name: "_dp_cell__dp_pc".to_string(),
                init: crate::basic_block::lowered_ir::ClosureInit::RuntimePcUnstarted,
            }],
        };

        let block = build_closure_backed_generator_factory_block(
            "_dp_bb_demo_factory",
            "_dp_bb_demo_0",
            crate::basic_block::lowered_ir::FunctionId(0),
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
        let context = Context::new(crate::basic_block::ast_to_ast::Options::for_test(), "");

        let entry = lower_if_stmt_sequence(
            &context,
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
        let context = Context::new(crate::basic_block::ast_to_ast::Options::for_test(), "");
        let entry = emit_sequence_return_block_with_expr_setup(
            &context,
            &mut blocks,
            "ret_label".to_string(),
            vec![py_stmt!("prefix = 0")],
            Some(py_expr!("value")),
        )
        .expect("sequence return helper should lower");

        assert_eq!(entry, "ret_label");
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0].term, BlockPyTerm::Return(Some(_))));
    }

    #[test]
    fn sequence_raise_helper_emits_raise_block() {
        let mut blocks = Vec::new();
        let context = Context::new(crate::basic_block::ast_to_ast::Options::for_test(), "");
        let entry = emit_sequence_raise_block_with_expr_setup(
            &context,
            &mut blocks,
            "raise_label".to_string(),
            vec![py_stmt!("prefix = 0")],
            BlockPyRaise {
                exc: Some(py_expr!("exc").into()),
            },
        )
        .expect("sequence raise helper should lower");

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
        let context = Context::new(crate::basic_block::ast_to_ast::Options::for_test(), "");

        let entry = lower_if_stmt_sequence_from_stmt(
            &context,
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
        let context = Context::new(crate::basic_block::ast_to_ast::Options::for_test(), "");

        let entry = lower_while_stmt_sequence(
            &context,
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
        let context = Context::new(crate::basic_block::ast_to_ast::Options::for_test(), "");

        let entry = lower_while_stmt_sequence_from_stmt(
            &context,
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
    fn lowers_assert_if_it_reaches_blockpy_stmt_lowering() {
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
        let context = test_context();
        let mut out = crate::basic_block::block_py::BlockPyCfgFragmentBuilder::<
            BlockPyStmt,
            BlockPyTerm,
        >::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(
            &context,
            func.body.body[0].as_ref(),
            &mut out,
            None,
            &mut next_label_id,
        )
        .expect("assert lowering should succeed");
        let fragment = out.finish();
        assert!(matches!(fragment.body.as_slice(), [BlockPyStmt::If(_)]));
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
    fn lowers_augassign_if_it_reaches_blockpy_stmt_lowering() {
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
        let context = test_context();
        let mut out = crate::basic_block::block_py::BlockPyCfgFragmentBuilder::<
            BlockPyStmt,
            BlockPyTerm,
        >::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(
            &context,
            func.body.body[0].as_ref(),
            &mut out,
            None,
            &mut next_label_id,
        )
        .expect("augassign lowering should succeed");
        let fragment = out.finish();
        assert!(matches!(fragment.body.as_slice(), [BlockPyStmt::Assign(_)]));
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
    fn lowers_typealias_if_it_reaches_blockpy_stmt_lowering() {
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
        let context = test_context();
        let mut out = crate::basic_block::block_py::BlockPyCfgFragmentBuilder::<
            BlockPyStmt,
            BlockPyTerm,
        >::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(
            &context,
            module.body[0].as_ref(),
            &mut out,
            None,
            &mut next_label_id,
        )
        .expect("type alias lowering should succeed");
        let fragment = out.finish();
        assert!(!fragment.body.is_empty());
    }

    #[test]
    fn lowers_match_if_it_reaches_blockpy_stmt_lowering() {
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
        let context = test_context();
        let mut out = crate::basic_block::block_py::BlockPyCfgFragmentBuilder::<
            BlockPyStmt,
            BlockPyTerm,
        >::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(
            &context,
            func.body.body[0].as_ref(),
            &mut out,
            None,
            &mut next_label_id,
        )
        .expect("match lowering should succeed");
        let fragment = out.finish();
        assert!(!fragment.body.is_empty() || fragment.term.is_some());
    }

    #[test]
    fn lowers_plain_import_if_it_reaches_blockpy_stmt_lowering() {
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
        let context = test_context();
        let mut out = crate::basic_block::block_py::BlockPyCfgFragmentBuilder::<
            BlockPyStmt,
            BlockPyTerm,
        >::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(
            &context,
            func.body.body[0].as_ref(),
            &mut out,
            None,
            &mut next_label_id,
        )
        .expect("import lowering should succeed");
        let fragment = out.finish();
        assert!(matches!(fragment.body.as_slice(), [BlockPyStmt::Assign(_)]));
    }

    #[test]
    fn lowers_importfrom_if_it_reaches_blockpy_stmt_lowering() {
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
        let context = test_context();
        let mut out = crate::basic_block::block_py::BlockPyCfgFragmentBuilder::<
            BlockPyStmt,
            BlockPyTerm,
        >::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(
            &context,
            func.body.body[0].as_ref(),
            &mut out,
            None,
            &mut next_label_id,
        )
        .expect("import-from lowering should succeed");
        let fragment = out.finish();
        assert!(!fragment.body.is_empty());
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
        let context = test_context();
        let mut out = crate::basic_block::block_py::BlockPyCfgFragmentBuilder::<
            BlockPyStmt,
            BlockPyTerm,
        >::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(
            &context,
            &Stmt::While(while_stmt.clone()),
            &mut out,
            None,
            &mut next_label_id,
        )
        .unwrap();
    }
}
