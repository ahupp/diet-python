use super::await_lower::{coroutine_generator_marker_stmt, lower_coroutine_awaits_to_yield_from};
use super::bb_ir::{BbExpr, BbFunction, BbFunctionKind, BbModule, BbOp, BindingTarget};
use super::block_py::{BlockPyBlock, BlockPyIf, BlockPyLabel, BlockPyStmt};
use super::ruff_to_blockpy::{
    blockpy_kind_for_lowered_runtime, build_closure_backed_generator_factory_block,
    build_for_target_assign_body,
    build_finalized_blockpy_function, compute_blockpy_exception_edges,
    drive_stmt_sequence_until_control, emit_sequence_jump_block, lower_common_stmt_sequence_head,
    lower_for_loop_continue_entry_with_state, lower_for_stmt_sequence_head,
    lower_generator_stmt_sequence_head,
    lower_generator_yield_terms_to_explicit_return_blockpy, lower_stmts_to_blockpy_stmts,
    lower_try_stmt_sequence_head, BlockPySequenceGeneratorState,
    GeneratorStmtSequenceLoweringState, GeneratorYieldSite, PendingBlockPyGeneratorInfo,
    StmtSequenceDriveResult, StmtSequenceHeadPlan,
};
use crate::template::into_body;
use crate::transform::context::Context;
use crate::transform::driver::SimplifyExprPass;
use crate::transform::rewrite_import;
use crate::transform::scope::{
    analyze_module_scope, cell_name, is_internal_symbol, BindingKind, BindingUse, Scope, ScopeKind,
};
use crate::transform::{
    ast_rewrite::{rewrite_with_pass, Rewrite, StmtRewritePass},
    rewrite_stmt,
};
use crate::transformer::{walk_expr, walk_stmt, Transformer};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, name::Name, Expr, NodeIndex, Stmt, StmtBody};
use ruff_text_size::TextRange;
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

mod annotation_helpers;
mod bound_names;
mod dataflow;
mod deleted_names;
mod exception_flow;
mod generator_closure;
mod lowering_helpers;
mod metadata;
mod naming;
mod pre_lower;
mod state_vars;
mod stmt_shape;
mod support;
mod symbol_analysis;
mod terminator_lowering;

use annotation_helpers::{
    annotation_helper_exec_binding_stmt, collect_capture_names, ensure_capture_default_params,
    ensure_dp_default_param, is_annotation_helper_name, render_stmt_source,
    rewrite_annotation_helper_defs_as_exec_calls, should_keep_non_lowered_for_annotationlib,
};
use bound_names::{collect_bound_names, collect_explicit_global_or_nonlocal_names};
use dataflow::{
    build_extra_successors_blockpy, compute_block_params_blockpy,
    ensure_try_exception_params_blockpy,
};
use deleted_names::{
    collect_deleted_names, rewrite_delete_to_deleted_sentinel, rewrite_deleted_name_loads,
};
pub(crate) use exception_flow::compute_exception_edge_by_label_blockpy;
pub(crate) use exception_flow::rewrite_region_returns_to_finally_blockpy as rewrite_region_returns_to_finally_blockpy_shared;
pub(crate) use exception_flow::{contains_return_stmt_in_body, contains_return_stmt_in_handlers};
use generator_closure::{
    build_generator_closure_layout, closure_backed_generator_resume_state_order,
};
pub(crate) use lowering_helpers::rewrite_exception_accesses as rewrite_exception_accesses_shared;
use lowering_helpers::{make_dp_tuple, make_param_specs_expr, name_expr};
use metadata::{
    collect_function_identity_private, display_name_for_function, function_annotation_entries,
    function_docstring_expr, split_docstring, FunctionIdentity,
};
pub(crate) use naming::{
    fold_constant_brif_blockpy, fold_jumps_to_trivial_none_return_blockpy,
    prune_unreachable_blockpy_blocks, relabel_blockpy_blocks,
};
use naming::{original_function_name, sanitize_ident};
use pre_lower::AnnotationHelperForLoweringPass;
pub use pre_lower::{BBSimplifyStmtPass, FunctionIdentityByNode};
pub(crate) use state_vars::sync_target_cells_stmts as sync_target_cells_stmts_shared;
use state_vars::{
    collect_cell_slots, collect_injected_exception_names_blockpy, collect_parameter_names,
    collect_state_vars, rewrite_sync_generator_blockpy_blocks_to_closure_cells,
    sync_generator_cleanup_cells, sync_generator_state_order, sync_target_cells_stmts,
};
use stmt_shape::{should_strip_nonlocal_for_bb, strip_nonlocal_directives};
pub(crate) use stmt_shape::{flatten_stmt, flatten_stmt_boxes};
use support::{
    has_await_in_stmts, has_dead_stmt_suffixes, has_yield_exprs_in_stmts, is_module_init_temp_name,
    prune_dead_stmt_suffixes, BasicBlockSupportChecker,
};
use terminator_lowering::bb_function_kind_from;

pub fn collect_function_identity_by_node(
    module: &mut StmtBody,
    module_scope: Arc<Scope>,
) -> FunctionIdentityByNode {
    collect_function_identity_private(module, module_scope)
        .into_iter()
        .map(|(node, identity)| {
            (
                node,
                (
                    identity.bind_name,
                    identity.display_name,
                    identity.qualname,
                    identity.binding_target,
                ),
            )
        })
        .collect()
}

pub fn rewrite_with_function_identity_and_collect_ir(
    context: &Context,
    module: &mut StmtBody,
    function_identity_by_node: FunctionIdentityByNode,
) -> BbModule {
    rewrite_internal(context, module, Some(function_identity_by_node))
}

fn rewrite_internal(
    context: &Context,
    module: &mut StmtBody,
    function_identity_by_node: Option<FunctionIdentityByNode>,
) -> BbModule {
    let module_scope = analyze_module_scope(module);
    let function_identity_by_node =
        if let Some(function_identity_by_node) = function_identity_by_node {
            function_identity_by_node
                .into_iter()
                .map(
                    |(node, (bind_name, display_name, qualname, binding_target))| {
                        (
                            node,
                            FunctionIdentity {
                                bind_name,
                                display_name,
                                qualname,
                                binding_target,
                            },
                        )
                    },
                )
                .collect()
        } else {
            collect_function_identity_private(module, module_scope.clone())
        };

    let mut rewriter = BasicBlockRewriter {
        context,
        module_scope,
        function_identity_by_node,
        next_block_id: 0,
        used_label_prefixes: HashMap::new(),
        function_stack: Vec::new(),
        function_cell_bindings_stack: Vec::new(),
        lower_stmt_sequence_cache: HashMap::new(),
        module_init_hoisted_blocks: Vec::new(),
        lowered_functions_ir: Vec::new(),
        module_init_function: Some("_dp_module_init".to_string()),
    };
    rewriter.visit_body(module);
    // BB lowering hoists nested lowered block functions into module-init and
    // leaves placeholder `pass` statements at original def sites. Strip them.
    crate::transform::simplify::strip_generated_passes(context, module);
    BbModule {
        functions: rewriter.lowered_functions_ir,
        module_init: Some("_dp_module_init".to_string()),
    }
}

struct BasicBlockRewriter<'a> {
    context: &'a Context,
    module_scope: Arc<Scope>,
    function_identity_by_node: HashMap<NodeIndex, FunctionIdentity>,
    next_block_id: usize,
    used_label_prefixes: HashMap<String, usize>,
    function_stack: Vec<String>,
    function_cell_bindings_stack: Vec<HashSet<String>>,
    lower_stmt_sequence_cache: HashMap<StmtSequenceCacheKey, String>,
    module_init_hoisted_blocks: Vec<Vec<Stmt>>,
    lowered_functions_ir: Vec<BbFunction>,
    module_init_function: Option<String>,
}

struct LoopContext {
    continue_label: String,
    break_label: String,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct StmtSequenceCacheKey {
    fn_name: String,
    stmts_ptr: usize,
    stmts_len: usize,
    cont_label: String,
    loop_continue_label: Option<String>,
    loop_break_label: Option<String>,
}

impl BasicBlockRewriter<'_> {
    fn stmt_sequence_cache_key(
        fn_name: &str,
        stmts: &[Box<Stmt>],
        cont_label: &str,
        loop_ctx: Option<&LoopContext>,
    ) -> StmtSequenceCacheKey {
        StmtSequenceCacheKey {
            fn_name: fn_name.to_string(),
            stmts_ptr: stmts.as_ptr() as usize,
            stmts_len: stmts.len(),
            cont_label: cont_label.to_string(),
            loop_continue_label: loop_ctx.map(|ctx| ctx.continue_label.clone()),
            loop_break_label: loop_ctx.map(|ctx| ctx.break_label.clone()),
        }
    }

    fn cache_stmt_sequence_result(&mut self, key: &StmtSequenceCacheKey, label: &str) {
        let _ = (key, label);
    }

    fn next_temp(&mut self, prefix: &str) -> String {
        let current = self.next_block_id;
        self.next_block_id += 1;
        format!("_dp_{prefix}_{current}")
    }

    fn try_lower_function(&mut self, func: &ast::StmtFunctionDef) -> Option<LoweredFunction> {
        if should_keep_non_lowered_for_annotationlib(func) {
            return None;
        }
        if func.name.id.as_str().starts_with("_dp_bb_") {
            return None;
        }
        let is_generated_genexpr = func.name.id.as_str().contains("_dp_genexpr_");
        let is_generated_comprehension_helper = is_generated_genexpr
            || func.name.id.as_str().contains("_dp_listcomp_")
            || func.name.id.as_str().contains("_dp_setcomp_")
            || func.name.id.as_str().contains("_dp_dictcomp_");
        // Keep generated annotation helpers in their lexical scope. BB-lowering
        // and hoisting them out of class/module init can break name resolution
        // for class-local symbols (for example, `T` in `value: T`).
        if is_annotation_helper_name(func.name.id.as_str()) {
            return None;
        }
        let (_, lowered_input_body) = split_docstring(&func.body);
        let lowered_input_body = flatten_stmt_boxes(&lowered_input_body);
        let lowered_input_body =
            if should_strip_nonlocal_for_bb(func.name.id.as_str()) || is_generated_genexpr {
                strip_nonlocal_directives(lowered_input_body)
            } else {
                lowered_input_body
            };
        let param_names = collect_parameter_names(&func.parameters);
        let has_yield_original = has_yield_exprs_in_stmts(&lowered_input_body);
        let mut runtime_input_body = prune_dead_stmt_suffixes(&lowered_input_body);
        let original_runtime_input_body = runtime_input_body.clone();
        // Keep await->yield-from lowering in the dedicated async pass for all
        // async functions so no `await` reaches BB IR/JIT planning.
        if func.is_async {
            lower_coroutine_awaits_to_yield_from(&mut runtime_input_body);
            let mut simplified_body = stmt_body_from_stmts(
                runtime_input_body
                    .iter()
                    .map(|stmt| stmt.as_ref().clone())
                    .collect(),
            );
            rewrite_with_pass(
                self.context,
                Some(&BBSimplifyStmtPass),
                Some(&SimplifyExprPass),
                &mut simplified_body,
            );
            runtime_input_body = flatten_stmt_boxes(&simplified_body.body);
        }
        let mut coroutine_via_generator = func.is_async && !has_yield_original;
        if coroutine_via_generator {
            if has_await_in_stmts(&runtime_input_body) {
                coroutine_via_generator = false;
                runtime_input_body = original_runtime_input_body;
            } else if !has_yield_exprs_in_stmts(&runtime_input_body) {
                runtime_input_body.insert(0, coroutine_generator_marker_stmt());
            }
        }
        let mut outer_scope_names = collect_bound_names(&runtime_input_body);
        outer_scope_names.extend(param_names.iter().cloned());
        let runtime_input_body =
            rewrite_annotation_helper_defs_as_exec_calls(runtime_input_body, &outer_scope_names);
        let mut outer_scope_names = collect_bound_names(&runtime_input_body);
        outer_scope_names.extend(param_names.iter().cloned());
        let unbound_local_names = if has_dead_stmt_suffixes(&lowered_input_body) {
            self.always_unbound_local_names(&lowered_input_body, &runtime_input_body, &param_names)
        } else {
            HashSet::new()
        };
        let deleted_names = collect_deleted_names(&runtime_input_body);
        let mut cell_slots = collect_cell_slots(&runtime_input_body);
        let has_yield = has_yield_exprs_in_stmts(&runtime_input_body);
        let has_await = has_await_in_stmts(&runtime_input_body);
        if func.is_async && has_await {
            return None;
        }
        if has_yield && has_await && !func.is_async {
            return None;
        }
        if !has_yield {
            let mut checker = BasicBlockSupportChecker {
                allow_await: func.is_async,
                ..Default::default()
            };
            let mut body_for_check = stmt_body_from_stmts(
                runtime_input_body
                    .iter()
                    .map(|stmt| stmt.as_ref().clone())
                    .collect(),
            );
            checker.visit_body(&mut body_for_check);
            if !checker.supported {
                return None;
            }
        }
        let is_async_generator_runtime = func.is_async && !coroutine_via_generator;
        // Generated async comprehension helpers still stay on the legacy
        // frame-backed resume path for now: forcing them onto the
        // closure-backed factory/resume path can blow up the helper plan size.
        // Sync generated genexpr helpers can use the normal closure-backed
        // generator runtime and should not keep the legacy binder path alive.
        let is_closure_backed_generator_runtime =
            has_yield && !(is_generated_comprehension_helper && func.is_async);

        self.lower_stmt_sequence_cache.clear();
        let end_label = self.next_label(func.name.id.as_str());
        let identity = self.function_identity_for(func);
        let blockpy_kind = blockpy_kind_for_lowered_runtime(
            is_async_generator_runtime,
            coroutine_via_generator,
            has_yield,
        );
        let mut blocks: Vec<BlockPyBlock> = Vec::new();
        let mut generator_state = BlockPySequenceGeneratorState {
            closure_state: is_closure_backed_generator_runtime,
            resume_order: Vec::new(),
            yield_sites: Vec::new(),
        };
        let entry_label = self.lower_stmt_sequence(
            func.name.id.as_str(),
            &runtime_input_body,
            end_label.clone(),
            &mut blocks,
            None,
            &cell_slots,
            &outer_scope_names,
            &mut generator_state,
        );
        let label_prefix = self.next_label_prefix(func.name.id.as_str());
        let (mut blockpy_function, entry_label) = build_finalized_blockpy_function(
            identity.bind_name.clone(),
            identity.qualname.clone(),
            identity.binding_target,
            blockpy_kind,
            (*func.parameters).clone(),
            blocks,
            entry_label,
            end_label,
            label_prefix.as_str(),
            if has_yield {
                Some(PendingBlockPyGeneratorInfo {
                    closure_state: generator_state.closure_state,
                    resume_order: generator_state.resume_order,
                    yield_sites: generator_state.yield_sites,
                })
            } else {
                None
            },
            is_async_generator_runtime,
            is_closure_backed_generator_runtime,
            self.next_temp("uncaught_exc"),
        );

        let exception_edges = compute_blockpy_exception_edges(&blockpy_function);

        let mut extra_successors = build_extra_successors_blockpy(&blockpy_function.blocks);
        let mut blocks_for_dataflow = std::mem::take(&mut blockpy_function.blocks);

        if !deleted_names.is_empty() {
            rewrite_deleted_name_loads(
                &mut blocks_for_dataflow,
                &deleted_names,
                &unbound_local_names,
            );
        } else if !unbound_local_names.is_empty() {
            rewrite_deleted_name_loads(
                &mut blocks_for_dataflow,
                &HashSet::new(),
                &unbound_local_names,
            );
        }
        let mut state_vars = collect_state_vars(
            &param_names,
            &blocks_for_dataflow,
            is_module_init_temp_name(func.name.id.as_str()),
        );
        let mut generator_capture_names = Vec::new();
        if is_closure_backed_generator_runtime {
            // Closure-backed generator state should include every bound local
            // from the source body, not only names inferred live from the BB
            // graph. This keeps coroutine-via-generator lowering aligned with
            // the "lift all locals" storage model, while still unioning in any
            // synthetic temps introduced during BB lowering.
            let mut bound_names = collect_bound_names(&runtime_input_body)
                .into_iter()
                .collect::<Vec<_>>();
            bound_names.sort();
            for name in bound_names {
                if !state_vars.iter().any(|existing| existing == &name) {
                    state_vars.push(name);
                }
            }
            let enclosing_scope = self
                .module_scope
                .child_scope_for_function(func)
                .ok()
                .and_then(|scope| scope.parent_scope());
            let enclosing_function_scope_names = enclosing_scope.and_then(|parent| {
                if matches!(parent.kind(), ScopeKind::Module)
                    || is_module_init_temp_name(parent.qualnamer.qualname.as_str())
                {
                    None
                } else {
                    Some(
                        parent
                            .scope_bindings()
                            .keys()
                            .cloned()
                            .collect::<HashSet<_>>(),
                    )
                }
            });
            generator_capture_names =
                collect_capture_names(func, enclosing_function_scope_names.as_ref());
            generator_capture_names.sort();
            generator_capture_names.dedup();
            for capture_name in &generator_capture_names {
                if !state_vars.iter().any(|existing| existing == capture_name) {
                    state_vars.push(capture_name.clone());
                }
            }
            for exc_name in collect_injected_exception_names_blockpy(&blocks_for_dataflow) {
                if !state_vars.iter().any(|existing| existing == &exc_name) {
                    state_vars.push(exc_name);
                }
            }
        }
        if is_closure_backed_generator_runtime {
            if !state_vars.iter().any(|existing| existing == "_dp_pc") {
                state_vars.push("_dp_pc".to_string());
            }
            if !state_vars
                .iter()
                .any(|existing| existing == "_dp_yieldfrom")
            {
                state_vars.push("_dp_yieldfrom".to_string());
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
        for (source, (target, _)) in &exception_edges {
            let Some(target) = target.as_ref() else {
                continue;
            };
            let successors = extra_successors.entry(source.clone()).or_default();
            if !successors.iter().any(|existing| existing == target) {
                successors.push(target.clone());
            }
        }
        if has_yield && !is_closure_backed_generator_runtime {
            for site in blockpy_function
                .generator
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
        if has_yield {
            // Generator/async-generator runtime dispatch passes state through
            // block args; keep `_dp_self` threaded even when local liveness
            // for a specific block would otherwise drop it.
            //
            // Active exception state from try/yield-from lowering must remain
            // available across resumed generator blocks even when it is only
            // referenced on exceptional control-flow paths. Treat all injected
            // exception names uniformly here instead of only `_dp_try_exc_*`.
            let injected_exc_names: Vec<String> =
                collect_injected_exception_names_blockpy(&blocks_for_dataflow)
                    .into_iter()
                    .collect();
            if is_closure_backed_generator_runtime {
                for block in &blocks_for_dataflow {
                    if !blockpy_function
                        .generator
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
                    if blockpy_function
                        .generator
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
                        == blockpy_function
                            .generator
                            .as_ref()
                            .and_then(|info| info.dispatch_entry_label.as_ref())
                            .map(BlockPyLabel::as_str)
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
                        let stmt = injected.pop().expect(
                            "generated deleted-sentinel init should yield one BlockPy stmt",
                        );
                        entry_block.body.insert(0, stmt);
                    }
                }
            }
        }
        let state_entry_label = blockpy_function
            .generator
            .as_ref()
            .and_then(|info| info.dispatch_entry_label.as_ref())
            .map(BlockPyLabel::as_str)
            .unwrap_or(entry_label.as_str())
            .to_string();
        ensure_try_exception_params_blockpy(&blocks_for_dataflow, &mut block_params);
        if is_closure_backed_generator_runtime {
            rewrite_sync_generator_blockpy_blocks_to_closure_cells(
                &mut blocks_for_dataflow,
                &mut block_params,
                &state_vars,
                &mut cell_slots,
                state_entry_label.as_str(),
            );
        }
        let injected_exception_names =
            collect_injected_exception_names_blockpy(&blocks_for_dataflow);
        let generator_closure_layout = if is_closure_backed_generator_runtime {
            Some(build_generator_closure_layout(
                &param_names,
                &state_vars,
                &generator_capture_names,
                &injected_exception_names,
            ))
        } else {
            None
        };
        let cleanup_cells = if is_closure_backed_generator_runtime {
            sync_generator_cleanup_cells(&state_vars, &injected_exception_names)
        } else {
            Vec::new()
        };
        if let (Some(uncaught_label), Some(uncaught_exc_name)) = (
            blockpy_function
                .generator
                .as_ref()
                .and_then(|info| info.uncaught_block_label.as_ref()),
            blockpy_function
                .generator
                .as_ref()
                .and_then(|info| info.uncaught_exc_name.as_ref()),
        ) {
            let params = block_params
                .entry(uncaught_label.as_str().to_string())
                .or_default();
            params.retain(|name| name != uncaught_exc_name);
            params.push(uncaught_exc_name.clone());
            if let Some(uncaught_set_done_label) = blockpy_function
                .generator
                .as_ref()
                .and_then(|info| info.uncaught_set_done_label.as_ref())
            {
                let params = block_params
                    .entry(uncaught_set_done_label.as_str().to_string())
                    .or_default();
                params.retain(|name| name != uncaught_exc_name);
                params.push(uncaught_exc_name.clone());
            }
            if let Some(uncaught_raise_label) = blockpy_function
                .generator
                .as_ref()
                .and_then(|info| info.uncaught_raise_label.as_ref())
            {
                let params = block_params
                    .entry(uncaught_raise_label.as_str().to_string())
                    .or_default();
                params.retain(|name| name != uncaught_exc_name);
                params.push(uncaught_exc_name.clone());
            }
        }
        if is_closure_backed_generator_runtime {
            if let Some(uncaught_set_done_label) = blockpy_function
                .generator
                .as_ref()
                .and_then(|info| info.uncaught_set_done_label.as_ref())
            {
                if let Some(uncaught_set_done_block) = blocks_for_dataflow
                    .iter_mut()
                    .find(|block| block.label.as_str() == uncaught_set_done_label.as_str())
                {
                    let mut new_body = Vec::with_capacity(
                        uncaught_set_done_block.body.len() + cleanup_cells.len(),
                    );
                    for stmt in std::mem::take(&mut uncaught_set_done_block.body) {
                        new_body.push(stmt);
                        if matches!(new_body.last(), Some(BlockPyStmt::Expr(_)))
                            && new_body.len() == 1
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
        let extra_state_vars: Vec<String> = if is_closure_backed_generator_runtime {
            Vec::new()
        } else {
            entry_params
                .iter()
                .filter(|name| !param_names.iter().any(|param| param == *name))
                .cloned()
                .collect()
        };
        let target_labels = blocks_for_dataflow
            .iter()
            .map(|block| block.label.as_str().to_string())
            .collect::<Vec<_>>();
        let resume_pcs = if has_yield {
            blockpy_function
                .generator
                .as_ref()
                .map(|info| info.resume_order.as_slice())
                .unwrap_or(&[])
                .iter()
                .enumerate()
                .map(|(idx, label)| (label.as_str().to_string(), idx))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let generator_yield_sites_for_lowering = blockpy_function
            .generator
            .as_ref()
            .map(|info| {
                info.yield_sites
                    .iter()
                    .map(|site| GeneratorYieldSite {
                        yield_label: site.yield_label.as_str().to_string(),
                        resume_label: site.resume_label.as_str().to_string(),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if has_yield {
            lower_generator_yield_terms_to_explicit_return_blockpy(
                &mut blocks_for_dataflow,
                &block_params,
                &resume_pcs,
                &generator_yield_sites_for_lowering,
                &cleanup_cells,
                is_async_generator_runtime,
                is_closure_backed_generator_runtime,
            );
        }
        let lowered_is_async = is_async_generator_runtime;
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
            let layout = generator_closure_layout
                .as_ref()
                .expect("closure-backed generator lowering requires closure layout");
            let factory_label = format!("{label_prefix}_factory");
            let factory_entry_params =
                closure_backed_generator_factory_entry_params(&param_names, layout);
            let resume_state_order =
                closure_backed_generator_resume_state_order(layout, lowered_is_async);
            let factory_block = build_closure_backed_generator_factory_block(
                factory_label.as_str(),
                entry_label.as_str(),
                &resume_state_order,
                identity.display_name.as_str(),
                identity.qualname.as_str(),
                layout,
                coroutine_via_generator,
                lowered_is_async,
            );
            let mut resume_local_cell_slots = cell_slots.iter().cloned().collect::<Vec<_>>();
            resume_local_cell_slots.sort();
            let resume_kind = if lowered_is_async {
                LoweredKind::AsyncGenerator {
                    closure_state: true,
                    resume_label: entry_label.clone(),
                    target_labels: target_labels.clone(),
                    resume_pcs: resume_pcs.clone(),
                }
            } else {
                LoweredKind::Generator {
                    closure_state: true,
                    resume_label: entry_label.clone(),
                    target_labels: target_labels.clone(),
                    resume_pcs: resume_pcs.clone(),
                }
            };
            helper_functions.push(BbFunction {
                bind_name: format!("{}_resume", identity.bind_name),
                display_name: "_dp_resume".to_string(),
                qualname: identity.qualname.clone(),
                binding_target: BindingTarget::Local,
                is_coroutine: false,
                kind: bb_function_kind_from(&resume_kind),
                entry: entry_label.clone(),
                param_names: param_names.clone(),
                entry_params: resume_state_order.clone(),
                generator_closure_layout: generator_closure_layout.clone(),
                param_specs: BbExpr::from_expr(closure_backed_generator_resume_param_specs_expr(
                    lowered_is_async,
                )),
                local_cell_slots: resume_local_cell_slots,
                blocks: super::blockpy_to_bb::lower_blockpy_blocks_to_bb_blocks(
                    self.context,
                    &exported_blocks,
                    &exported_block_params,
                    &exported_exception_edges,
                ),
            });
            exported_blocks = vec![factory_block];
            exported_entry_label = factory_label;
            exported_entry_params = factory_entry_params;
            exported_block_params =
                HashMap::from([(exported_entry_label.clone(), exported_entry_params.clone())]);
            exported_exception_edges = HashMap::new();
        }
        Some(LoweredFunction {
            blocks: exported_blocks,
            entry_label: exported_entry_label,
            entry_params: exported_entry_params,
            block_params: exported_block_params,
            exception_edges: exported_exception_edges,
            generator_closure_layout: if is_closure_backed_generator_runtime {
                None
            } else {
                generator_closure_layout
            },
            local_cell_slots: cell_slots.clone(),
            param_specs: BbExpr::from_expr(make_param_specs_expr(func.parameters.as_ref())),
            param_names,
            coroutine_wrapper: coroutine_via_generator,
            kind: if has_yield && !is_closure_backed_generator_runtime {
                if lowered_is_async {
                    LoweredKind::AsyncGenerator {
                        closure_state: is_closure_backed_generator_runtime,
                        resume_label: entry_label.clone(),
                        target_labels,
                        resume_pcs,
                    }
                } else {
                    LoweredKind::Generator {
                        closure_state: is_closure_backed_generator_runtime,
                        resume_label: entry_label.clone(),
                        target_labels,
                        resume_pcs,
                    }
                }
            } else {
                LoweredKind::Function
            },
            helper_functions,
        })
    }

    fn lower_stmt_sequence(
        &mut self,
        fn_name: &str,
        stmts: &[Box<Stmt>],
        cont_label: String,
        blocks: &mut Vec<BlockPyBlock>,
        loop_ctx: Option<&LoopContext>,
        cell_slots: &HashSet<String>,
        outer_scope_names: &HashSet<String>,
        generator_state: &mut BlockPySequenceGeneratorState,
    ) -> String {
        if stmts.is_empty() {
            return cont_label;
        }

        let cache_key = Self::stmt_sequence_cache_key(fn_name, stmts, &cont_label, loop_ctx);

        let mut linear = Vec::new();
        let mut index = 0;
        while index < stmts.len() {
            let plan;
            (linear, index, plan) = match drive_stmt_sequence_until_control(
                &stmts[index..],
                linear,
                &mut |func_def| {
                    self.lower_non_bb_def_stmt_to_exec_binding(
                        func_def,
                        cell_slots,
                        outer_scope_names,
                    )
                },
                &mut |delete_stmt| rewrite_delete_to_deleted_sentinel(delete_stmt),
            ) {
                StmtSequenceDriveResult::Exhausted { linear } => {
                    let label = self.next_label(fn_name);
                    let label = emit_sequence_jump_block(blocks, label, linear, cont_label);
                    self.cache_stmt_sequence_result(&cache_key, &label);
                    return label;
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
                        closure_state: generator_state.closure_state,
                        resume_order: std::mem::take(&mut generator_state.resume_order),
                        yield_sites: std::mem::take(&mut generator_state.yield_sites),
                        next_block_id: self.next_block_id,
                    };
                    let (label, updated_state) = lower_generator_stmt_sequence_head(
                        fn_name,
                        plan,
                        &stmts[index + 1..],
                        cont_label.clone(),
                        linear.clone(),
                        blocks,
                        initial_state,
                        sync_target_cells.then_some(cell_slots),
                        &mut |stmts, cont_label, blocks, state| {
                            let mut local_generator_state = BlockPySequenceGeneratorState {
                                closure_state: state.closure_state,
                                resume_order: state.resume_order,
                                yield_sites: state.yield_sites,
                            };
                            self.next_block_id = state.next_block_id;
                            let label = self.lower_stmt_sequence(
                                fn_name,
                                stmts,
                                cont_label,
                                blocks,
                                loop_ctx,
                                cell_slots,
                                outer_scope_names,
                                &mut local_generator_state,
                            );
                            (
                                label,
                                GeneratorStmtSequenceLoweringState {
                                    closure_state: local_generator_state.closure_state,
                                    resume_order: local_generator_state.resume_order,
                                    yield_sites: local_generator_state.yield_sites,
                                    next_block_id: self.next_block_id,
                                },
                            )
                        },
                    );
                    self.next_block_id = updated_state.next_block_id;
                    generator_state.resume_order = updated_state.resume_order;
                    generator_state.yield_sites = updated_state.yield_sites;
                    if let Some(label) = label {
                        self.cache_stmt_sequence_result(&cache_key, &label);
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
                    let sanitized_fn_name = sanitize_ident(fn_name);
                    let next_block_id = Cell::new(self.next_block_id);
                    let label = lower_common_stmt_sequence_head(
                        plan,
                        &stmts[index + 1..],
                        cont_label.clone(),
                        linear,
                        blocks,
                        &mut || {
                            let current = next_block_id.get();
                            next_block_id.set(current + 1);
                            format!("_dp_bb_{}_{}", sanitized_fn_name, current)
                        },
                        loop_ctx.map(|loop_ctx| loop_ctx.break_label.clone()),
                        loop_ctx.map(|loop_ctx| loop_ctx.continue_label.clone()),
                        &mut |stmts, cont_label, break_label, blocks| {
                            self.next_block_id = next_block_id.get();
                            if let Some(break_label) = break_label {
                                let loop_ctx = LoopContext {
                                    continue_label: cont_label.clone(),
                                    break_label,
                                };
                                let label = self.lower_stmt_sequence(
                                    fn_name,
                                    stmts,
                                    cont_label,
                                    blocks,
                                    Some(&loop_ctx),
                                    cell_slots,
                                    outer_scope_names,
                                    generator_state,
                                );
                                next_block_id.set(self.next_block_id);
                                label
                            } else {
                                let label = self.lower_stmt_sequence(
                                    fn_name,
                                    stmts,
                                    cont_label,
                                    blocks,
                                    loop_ctx,
                                    cell_slots,
                                    outer_scope_names,
                                    generator_state,
                                );
                                next_block_id.set(self.next_block_id);
                                label
                            }
                        },
                    );
                    self.next_block_id = next_block_id.get();
                    if let Some(label) = label {
                        self.cache_stmt_sequence_result(&cache_key, &label);
                        return label;
                    }
                    unreachable!("common head helper must lower supported head");
                }
                StmtSequenceHeadPlan::For(for_stmt) => {
                    let iter_name = self.next_temp("iter");
                    let tmp_name = self.next_temp("tmp");
                    let Some(tmp_expr) = name_expr(tmp_name.as_str()) else {
                        return cont_label;
                    };
                    let loop_check_label = self.next_label(fn_name);
                    let continue_state = GeneratorStmtSequenceLoweringState {
                        closure_state: generator_state.closure_state,
                        resume_order: std::mem::take(&mut generator_state.resume_order),
                        yield_sites: std::mem::take(&mut generator_state.yield_sites),
                        next_block_id: self.next_block_id,
                    };
                    let (loop_continue_label, continue_state) = lower_for_loop_continue_entry_with_state(
                        blocks,
                        fn_name,
                        iter_name.as_str(),
                        tmp_name.as_str(),
                        loop_check_label.clone(),
                        for_stmt.is_async,
                        continue_state,
                    );
                    self.next_block_id = continue_state.next_block_id;
                    generator_state.resume_order = continue_state.resume_order;
                    generator_state.yield_sites = continue_state.yield_sites;
                    let mut next_temp = |prefix: &str| self.next_temp(prefix);
                    let assign_body = build_for_target_assign_body(
                        for_stmt.target.as_ref(),
                        tmp_expr.clone(),
                        tmp_name.as_str(),
                        cell_slots,
                        &mut next_temp,
                    );
                    let next_block_id = Cell::new(self.next_block_id);
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
                        &next_block_id,
                        &mut |stmts, cont_label, break_label, blocks| {
                            self.next_block_id = next_block_id.get();
                            if let Some(break_label) = break_label {
                                let loop_ctx = LoopContext {
                                    continue_label: cont_label.clone(),
                                    break_label,
                                };
                                let label = self.lower_stmt_sequence(
                                    fn_name,
                                    stmts,
                                    cont_label,
                                    blocks,
                                    Some(&loop_ctx),
                                    cell_slots,
                                    outer_scope_names,
                                    generator_state,
                                );
                                next_block_id.set(self.next_block_id);
                                label
                            } else {
                                let label = self.lower_stmt_sequence(
                                    fn_name,
                                    stmts,
                                    cont_label,
                                    blocks,
                                    loop_ctx,
                                    cell_slots,
                                    outer_scope_names,
                                    generator_state,
                                );
                                next_block_id.set(self.next_block_id);
                                label
                            }
                        },
                    );
                    self.next_block_id = next_block_id.get();
                    self.cache_stmt_sequence_result(&cache_key, &label);
                    return label;
                }
                StmtSequenceHeadPlan::Try(try_stmt) => {
                    let next_block_id = Cell::new(self.next_block_id);
                    let label = lower_try_stmt_sequence_head(
                        fn_name,
                        try_stmt,
                        &stmts[index + 1..],
                        cont_label.clone(),
                        linear,
                        blocks,
                        &next_block_id,
                        &mut |stmts, cont_label, blocks| {
                            self.next_block_id = next_block_id.get();
                            let label = self.lower_stmt_sequence(
                                fn_name,
                                stmts,
                                cont_label,
                                blocks,
                                loop_ctx,
                                cell_slots,
                                outer_scope_names,
                                generator_state,
                            );
                            next_block_id.set(self.next_block_id);
                            label
                        },
                    );
                    self.next_block_id = next_block_id.get();
                    self.cache_stmt_sequence_result(&cache_key, &label);
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

        let label = self.next_label(fn_name);
        let label = emit_sequence_jump_block(blocks, label, linear, cont_label);
        self.cache_stmt_sequence_result(&cache_key, &label);
        label
    }

    fn lower_non_bb_def_stmt_to_exec_binding(
        &self,
        func_def: &ast::StmtFunctionDef,
        cell_slots: &HashSet<String>,
        outer_scope_names: &HashSet<String>,
    ) -> Vec<Stmt> {
        let mut source_fn = func_def.clone();
        let bind_name = source_fn.name.id.to_string();
        ensure_dp_default_param(&mut source_fn);
        let capture_names = collect_capture_names(&source_fn, Some(outer_scope_names));
        ensure_capture_default_params(&mut source_fn, &capture_names);
        let source = render_stmt_source(&Stmt::FunctionDef(source_fn));
        let captures = make_dp_tuple(
            capture_names
                .iter()
                .map(|name| {
                    py_expr!(
                        "({name:literal}, {value:id})",
                        name = name.as_str(),
                        value = name.as_str(),
                    )
                })
                .collect(),
        );
        let base_value = py_expr!(
            "__dp_exec_function_def_source({source:literal}, __dp_globals(), {captures:expr}, {name:literal})",
            source = source.as_str(),
            captures = captures,
            name = bind_name.as_str(),
        );
        let mut out = vec![py_stmt!(
            "{name:id} = {value:expr}",
            name = bind_name.as_str(),
            value = base_value,
        )];
        let target_expr = py_expr!("{name:id}", name = bind_name.as_str());
        out.extend(sync_target_cells_stmts(&target_expr, cell_slots));
        out
    }

    fn next_label(&mut self, fn_name: &str) -> String {
        let current = self.next_block_id;
        self.next_block_id += 1;
        format!("_dp_bb_{}_{}", sanitize_ident(fn_name), current)
    }

    fn next_label_prefix(&mut self, fn_name: &str) -> String {
        let base = sanitize_ident(original_function_name(fn_name).as_str());
        let count = self.used_label_prefixes.entry(base.clone()).or_insert(0);
        let suffix = if *count == 0 {
            String::new()
        } else {
            format!("_{}", *count)
        };
        *count += 1;
        format!("_dp_bb_{base}{suffix}")
    }

    fn function_identity_for(&self, func: &ast::StmtFunctionDef) -> FunctionIdentity {
        if is_module_init_temp_name(func.name.id.as_str()) {
            return FunctionIdentity {
                bind_name: "_dp_module_init".to_string(),
                display_name: "_dp_module_init".to_string(),
                qualname: "_dp_module_init".to_string(),
                binding_target: BindingTarget::ModuleGlobal,
            };
        }
        let node_index = func.node_index.load();
        if let Some(identity) = self.function_identity_by_node.get(&node_index) {
            let mut identity = identity.clone();
            if self
                .function_stack
                .last()
                .is_some_and(|parent| parent.starts_with("_dp_class_ns_"))
                && !is_internal_symbol(func.name.id.as_str())
            {
                identity.binding_target = BindingTarget::ClassNamespace;
            }
            return identity;
        }
        let bind_name = func.name.id.to_string();
        let display_name = display_name_for_function(bind_name.as_str()).to_string();
        FunctionIdentity {
            bind_name: bind_name.clone(),
            display_name,
            qualname: bind_name,
            binding_target: self.default_binding_target_for_name(func.name.id.as_str()),
        }
    }

    fn build_def_expr_from_bb(
        &self,
        bb_function: &BbFunction,
        doc_expr: Option<Expr>,
        annotate_fn_expr: Option<Expr>,
    ) -> Option<Expr> {
        let entry_label = bb_function.entry.as_str();
        let entry_ref_expr = py_expr!("{entry:literal}", entry = entry_label);
        let param_names: HashSet<&str> =
            bb_function.param_names.iter().map(String::as_str).collect();
        let generator_lifted_state_names: HashSet<&str> = bb_function
            .generator_closure_layout
            .as_ref()
            .map(|layout| {
                layout
                    .lifted_locals
                    .iter()
                    .chain(layout.runtime_cells.iter())
                    .map(|slot| slot.logical_name.as_str())
                    .collect()
            })
            .unwrap_or_default();
        let generator_closure_storage_names: HashSet<&str> = bb_function
            .generator_closure_layout
            .as_ref()
            .map(|layout| {
                layout
                    .inherited_captures
                    .iter()
                    .chain(layout.lifted_locals.iter())
                    .chain(layout.runtime_cells.iter())
                    .map(|slot| slot.storage_name.as_str())
                    .collect()
            })
            .unwrap_or_default();
        let locally_assigned: HashSet<&str> = bb_function
            .blocks
            .iter()
            .flat_map(|block| block.ops.iter())
            .filter_map(|op| match op {
                BbOp::Assign(assign) => Some(assign.target.id.as_str()),
                _ => None,
            })
            .collect();
        let mut closure_items = Vec::new();
        for entry_name in &bb_function.entry_params {
            if param_names.contains(entry_name.as_str()) {
                closure_items.push(py_expr!("{value:literal}", value = entry_name.as_str(),));
            } else if entry_name == "_dp_classcell"
                || (entry_name.starts_with("_dp_cell_")
                    && !bb_function
                        .local_cell_slots
                        .iter()
                        .any(|slot| slot == entry_name))
            {
                let value = name_expr(entry_name.as_str())?;
                closure_items.push(make_dp_tuple(vec![
                    py_expr!("{value:literal}", value = entry_name.as_str()),
                    value,
                ]));
            } else if matches!(
                &bb_function.kind,
                BbFunctionKind::Generator {
                    closure_state: true,
                    ..
                } | BbFunctionKind::AsyncGenerator {
                    closure_state: true,
                    ..
                }
            ) && generator_closure_storage_names.contains(entry_name.as_str())
            {
                let value = name_expr(entry_name.as_str())?;
                closure_items.push(make_dp_tuple(vec![
                    py_expr!("{value:literal}", value = entry_name.as_str()),
                    value,
                ]));
            } else if matches!(
                &bb_function.kind,
                BbFunctionKind::Generator {
                    closure_state: true,
                    ..
                } | BbFunctionKind::AsyncGenerator {
                    closure_state: true,
                    ..
                }
            ) && generator_lifted_state_names.contains(entry_name.as_str())
            {
                closure_items.push(py_expr!("{value:literal}", value = entry_name.as_str(),));
            } else if !entry_name.starts_with("_dp_")
                && !locally_assigned.contains(entry_name.as_str())
            {
                let value = name_expr(entry_name.as_str())?;
                closure_items.push(make_dp_tuple(vec![
                    py_expr!("{value:literal}", value = entry_name.as_str()),
                    value,
                ]));
            } else {
                closure_items.push(py_expr!("{value:literal}", value = entry_name.as_str(),));
            }
        }
        let closure = make_dp_tuple(closure_items);
        let doc = doc_expr.unwrap_or_else(|| py_expr!("None"));
        let annotate_fn = annotate_fn_expr.unwrap_or_else(|| py_expr!("None"));
        let function_entry_expr = py_expr!(
            "__dp_def_fn({entry:expr}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, {module_globals:expr}, {module_name:expr}, {doc:expr}, {annotate_fn:expr})",
            entry = entry_ref_expr.clone(),
            name = bb_function.display_name.as_str(),
            qualname = bb_function.qualname.as_str(),
            closure = closure.clone(),
            params = bb_function.param_specs.to_expr(),
            module_globals = py_expr!("__dp_globals()"),
            module_name = py_expr!("__name__"),
            doc = doc.clone(),
            annotate_fn = annotate_fn.clone(),
        );
        match &bb_function.kind {
            BbFunctionKind::Function => {
                if bb_function.is_coroutine {
                    Some(py_expr!(
                        "__dp_mark_coroutine_function({func:expr})",
                        func = function_entry_expr,
                    ))
                } else {
                    Some(function_entry_expr)
                }
            }
            BbFunctionKind::AsyncGenerator { closure_state, .. } => {
                if *closure_state {
                    return Some(function_entry_expr);
                }
                Some(py_expr!(
                    "__dp_def_async_gen({resume:expr}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, __dp_globals(), __name__, {doc:expr}, {annotate_fn:expr})",
                    resume = entry_ref_expr.clone(),
                    name = bb_function.display_name.as_str(),
                    qualname = bb_function.qualname.as_str(),
                    closure = closure,
                    params = bb_function.param_specs.to_expr(),
                    doc = doc.clone(),
                    annotate_fn = annotate_fn.clone(),
                ))
            }
            BbFunctionKind::Generator { closure_state, .. } => {
                if *closure_state {
                    if bb_function.is_coroutine {
                        return Some(py_expr!(
                            "__dp_mark_coroutine_function({func:expr})",
                            func = function_entry_expr,
                        ));
                    }
                    return Some(function_entry_expr);
                }
                if bb_function.is_coroutine {
                    Some(py_expr!(
                        "__dp_def_coro_from_gen({resume:expr}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, __dp_globals(), __name__, {doc:expr}, {annotate_fn:expr})",
                        resume = entry_ref_expr,
                        name = bb_function.display_name.as_str(),
                        qualname = bb_function.qualname.as_str(),
                        closure = closure,
                        params = bb_function.param_specs.to_expr(),
                        doc = doc,
                        annotate_fn = annotate_fn,
                    ))
                } else {
                    panic!(
                        "non-closure-backed sync generator lowering is unreachable; \
                         generated comprehension helpers are async-only"
                    )
                }
            }
        }
    }

    fn build_lowered_binding_stmt(
        &self,
        func: &ast::StmtFunctionDef,
        bb_function: &BbFunction,
    ) -> Option<Stmt> {
        let identity = self.function_identity_for(func);
        let target = self.resolved_binding_target(&identity);
        let bind_name = identity.bind_name.as_str();

        let annotation_entries = function_annotation_entries(func);
        let annotate_helper_stmt = if annotation_entries.is_empty() {
            None
        } else {
            // Keep helper name in __annotate__ family so BB lowering keeps it in lexical scope.
            let annotate_helper_name = format!("_dp_fn___annotate___{bind_name}");
            let helper_stmt = rewrite_stmt::annotation::build_annotate_fn(
                annotation_entries,
                annotate_helper_name.as_str(),
            );
            let helper_stmt = match helper_stmt {
                Stmt::FunctionDef(helper_fn) => annotation_helper_exec_binding_stmt(
                    helper_fn,
                    annotate_helper_name.as_str(),
                    None,
                ),
                other => other,
            };
            Some((annotate_helper_name.clone(), helper_stmt))
        };

        let annotate_fn_expr = match annotate_helper_stmt.as_ref() {
            Some((helper_name, _)) => Some(name_expr(helper_name.as_str())?),
            None => None,
        };
        let doc_expr = function_docstring_expr(func);

        let base_expr = self.build_def_expr_from_bb(bb_function, doc_expr, annotate_fn_expr)?;
        let decorated = rewrite_stmt::decorator::rewrite(func.decorator_list.clone(), base_expr);
        let binding_stmt = self.make_binding_stmt(target, bind_name, decorated);
        let mut stmts = Vec::new();
        if let Some((_, helper_stmt)) = annotate_helper_stmt {
            stmts.push(helper_stmt);
        }
        stmts.push(binding_stmt);
        if target == BindingTarget::Local && self.needs_cell_sync(bind_name) {
            let cell = cell_name(bind_name);
            stmts.push(py_stmt!(
                "__dp_store_cell({cell:id}, {name:id})",
                cell = cell.as_str(),
                name = bind_name,
            ));
        }
        if stmts.len() == 1 {
            stmts.into_iter().next()
        } else {
            Some(into_body(stmts))
        }
    }

    fn default_binding_target_for_name(&self, bind_name: &str) -> BindingTarget {
        match self.function_stack.last().map(String::as_str) {
            Some(parent) if is_module_init_temp_name(parent) => {
                if is_internal_symbol(bind_name) {
                    BindingTarget::Local
                } else {
                    BindingTarget::ModuleGlobal
                }
            }
            Some(parent) if parent.starts_with("_dp_class_ns_") => {
                if is_internal_symbol(bind_name) {
                    BindingTarget::Local
                } else {
                    BindingTarget::ClassNamespace
                }
            }
            _ => BindingTarget::Local,
        }
    }

    fn make_binding_stmt(&self, target: BindingTarget, bind_name: &str, value: Expr) -> Stmt {
        match target {
            BindingTarget::Local => {
                py_stmt!("{name:id} = {value:expr}", name = bind_name, value = value,)
            }
            BindingTarget::ModuleGlobal => py_stmt!(
                "__dp_store_global(globals(), {name:literal}, {value:expr})",
                name = bind_name,
                value = value,
            ),
            BindingTarget::ClassNamespace => py_stmt!(
                "__dp_setitem(_dp_class_ns, {name:literal}, {value:expr})",
                name = bind_name,
                value = value,
            ),
        }
    }

    fn needs_cell_sync(&self, bind_name: &str) -> bool {
        self.function_cell_bindings_stack
            .last()
            .map(|cells| cells.contains(bind_name))
            .unwrap_or(false)
    }

    fn resolved_binding_target(&self, identity: &FunctionIdentity) -> BindingTarget {
        if identity.binding_target == BindingTarget::Local
            && identity.qualname == identity.bind_name
            && !is_internal_symbol(identity.bind_name.as_str())
        {
            // Explicit `global` in nested scopes can still surface here as
            // local after lowering; global-qualname defs must bind to globals.
            BindingTarget::ModuleGlobal
        } else {
            identity.binding_target
        }
    }

    fn build_non_lowered_binding_stmt(&mut self, func: &mut ast::StmtFunctionDef) -> Option<Stmt> {
        let identity = self.function_identity_for(func);
        let bind_name = identity.bind_name.to_string();
        let target = self.resolved_binding_target(&identity);

        if target == BindingTarget::Local {
            if self.needs_cell_sync(bind_name.as_str()) {
                let cell = cell_name(bind_name.as_str());
                return Some(py_stmt!(
                    "__dp_store_cell({cell:id}, {name:id})",
                    cell = cell.as_str(),
                    name = bind_name.as_str(),
                ));
            }
            return None;
        }

        // For non-local bindings, define under an internal temporary name and
        // bind the user-visible name explicitly. This preserves class-scope
        // lookup semantics (`open` should resolve to builtins inside
        // `Wrapper.open`) and honors `global` directives in nested scopes.
        let mut local_name = func.name.id.to_string();
        if !is_internal_symbol(local_name.as_str())
            && !is_annotation_helper_name(bind_name.as_str())
        {
            local_name = self.next_temp("fn_local");
            func.name.id = Name::new(local_name.as_str());
        }

        let decorators = std::mem::take(&mut func.decorator_list);
        let doc = function_docstring_expr(func).unwrap_or_else(|| py_expr!("None"));
        let updated = py_expr!(
            "__dp_update_fn({name:id}, {qualname:literal}, {display_name:literal}, {doc:expr})",
            name = local_name.as_str(),
            qualname = identity.qualname.as_str(),
            display_name = identity.display_name.as_str(),
            doc = doc,
        );
        let value = rewrite_stmt::decorator::rewrite(decorators, updated);
        Some(self.make_binding_stmt(target, bind_name.as_str(), value))
    }

    fn always_unbound_local_names(
        &self,
        lowered_input_body: &[Box<Stmt>],
        runtime_body: &[Box<Stmt>],
        param_names: &[String],
    ) -> HashSet<String> {
        let original_bound_names = collect_bound_names(lowered_input_body);
        let runtime_bound_names = collect_bound_names(runtime_body);
        let explicit_global_or_nonlocal =
            collect_explicit_global_or_nonlocal_names(lowered_input_body);
        original_bound_names
            .into_iter()
            .filter_map(|name| {
                if param_names.iter().any(|param| param == &name) {
                    return None;
                }
                if is_internal_symbol(name.as_str()) {
                    return None;
                }
                if runtime_bound_names.contains(name.as_str()) {
                    return None;
                }
                if explicit_global_or_nonlocal.contains(name.as_str()) {
                    return None;
                }
                Some(name)
            })
            .collect()
    }
}

struct LoweredFunction {
    blocks: Vec<BlockPyBlock>,
    entry_label: String,
    entry_params: Vec<String>,
    block_params: HashMap<String, Vec<String>>,
    exception_edges: HashMap<String, (Option<String>, Option<String>)>,
    generator_closure_layout: Option<crate::basic_block::bb_ir::BbGeneratorClosureLayout>,
    local_cell_slots: HashSet<String>,
    param_specs: BbExpr,
    param_names: Vec<String>,
    coroutine_wrapper: bool,
    kind: LoweredKind,
    helper_functions: Vec<BbFunction>,
}

#[derive(Clone)]
enum LoweredKind {
    Function,
    AsyncGenerator {
        closure_state: bool,
        resume_label: String,
        target_labels: Vec<String>,
        resume_pcs: Vec<(String, usize)>,
    },
    Generator {
        closure_state: bool,
        resume_label: String,
        target_labels: Vec<String>,
        resume_pcs: Vec<(String, usize)>,
    },
}

impl Transformer for BasicBlockRewriter<'_> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if let Stmt::FunctionDef(func) = stmt {
            let fn_name = func.name.id.to_string();
            let entering_module_init = is_module_init_temp_name(fn_name.as_str());
            self.module_init_hoisted_blocks.push(Vec::new());
            let function_cell_bindings = collect_cell_slots(&func.body.body)
                .into_iter()
                .filter_map(|slot| slot.strip_prefix("_dp_cell_").map(str::to_string))
                .collect::<HashSet<_>>();
            self.function_stack.push(fn_name);
            self.function_cell_bindings_stack
                .push(function_cell_bindings);
            walk_stmt(self, stmt);
            self.function_stack.pop();
            self.function_cell_bindings_stack.pop();
            let mut function_hoisted = self.module_init_hoisted_blocks.pop().unwrap_or_default();

            if let Stmt::FunctionDef(func) = stmt {
                if let Some(lowered) = self.try_lower_function(func) {
                    let identity = self.function_identity_for(func);
                    let resolved_target = self.resolved_binding_target(&identity);
                    self.lowered_functions_ir
                        .extend(lowered.helper_functions.iter().cloned());
                    let mut local_cell_slots =
                        lowered.local_cell_slots.iter().cloned().collect::<Vec<_>>();
                    local_cell_slots.sort();
                    let bb_function = BbFunction {
                        bind_name: identity.bind_name.clone(),
                        display_name: identity.display_name.clone(),
                        qualname: identity.qualname.clone(),
                        binding_target: resolved_target,
                        is_coroutine: lowered.coroutine_wrapper,
                        kind: bb_function_kind_from(&lowered.kind),
                        entry: lowered.entry_label.clone(),
                        param_names: lowered.param_names.clone(),
                        entry_params: lowered.entry_params.clone(),
                        generator_closure_layout: lowered.generator_closure_layout.clone(),
                        param_specs: lowered.param_specs.clone(),
                        local_cell_slots,
                        blocks: super::blockpy_to_bb::lower_blockpy_blocks_to_bb_blocks(
                            self.context,
                            &lowered.blocks,
                            &lowered.block_params,
                            &lowered.exception_edges,
                        ),
                    };
                    self.lowered_functions_ir.push(bb_function.clone());
                    if self.module_init_function.is_none()
                        && identity.bind_name.as_str() == "_dp_module_init"
                    {
                        self.module_init_function = Some(identity.bind_name.clone());
                    }
                    let binding_stmt = self
                        .build_lowered_binding_stmt(func, &bb_function)
                        .expect("failed to build BB function binding");
                    let keep_local_blocks = !entering_module_init
                        && !self.module_init_hoisted_blocks.is_empty()
                        && (identity.bind_name.starts_with("_dp_class_ns_")
                            || identity.bind_name.starts_with("_dp_define_class_"));
                    if entering_module_init {
                        let mut lowered_defs = function_hoisted;
                        lowered_defs.push(binding_stmt);
                        *stmt = into_body(lowered_defs);
                    } else if keep_local_blocks {
                        let mut body = function_hoisted;
                        body.push(binding_stmt);
                        *stmt = into_body(body);
                    } else if !self.module_init_hoisted_blocks.is_empty() {
                        if let Some(hoisted) = self.module_init_hoisted_blocks.last_mut() {
                            hoisted.append(&mut function_hoisted);
                        }
                        *stmt = binding_stmt;
                    } else {
                        let mut body = function_hoisted;
                        body.push(binding_stmt);
                        *stmt = into_body(body);
                    }
                } else {
                    if should_keep_non_lowered_for_annotationlib(func) {
                        rewrite_with_pass(
                            self.context,
                            Some(&AnnotationHelperForLoweringPass),
                            None,
                            &mut func.body,
                        );
                        ensure_dp_default_param(func);
                    }
                    let non_lowered_binding = self.build_non_lowered_binding_stmt(func);
                    if let Some(binding_stmt) = non_lowered_binding {
                        let mut body = Vec::new();
                        body.append(&mut function_hoisted);
                        body.push(Stmt::FunctionDef(func.clone()));
                        body.push(binding_stmt);
                        *stmt = into_body(body);
                    } else if !function_hoisted.is_empty() {
                        let mut new_body = function_hoisted
                            .into_iter()
                            .map(Box::new)
                            .collect::<Vec<_>>();
                        new_body.extend(std::mem::take(&mut func.body.body));
                        func.body.body = new_body;
                    }
                }
            }
            return;
        }

        walk_stmt(self, stmt);
    }
}

fn closure_backed_generator_factory_entry_params(
    param_names: &[String],
    layout: &crate::basic_block::bb_ir::BbGeneratorClosureLayout,
) -> Vec<String> {
    let mut params = param_names.to_vec();
    for slot in &layout.inherited_captures {
        if !params.iter().any(|name| name == &slot.storage_name) {
            params.push(slot.storage_name.clone());
        }
    }
    params
}

fn closure_backed_generator_resume_param_specs_expr(is_async_generator: bool) -> Expr {
    let mut params = vec![
        make_dp_tuple(vec![
            py_expr!("{name:literal}", name = "/_dp_self"),
            py_expr!("None"),
            py_expr!("__dp_NO_DEFAULT"),
        ]),
        make_dp_tuple(vec![
            py_expr!("{name:literal}", name = "/_dp_send_value"),
            py_expr!("None"),
            py_expr!("__dp_NO_DEFAULT"),
        ]),
        make_dp_tuple(vec![
            py_expr!("{name:literal}", name = "/_dp_resume_exc"),
            py_expr!("None"),
            py_expr!("__dp_NO_DEFAULT"),
        ]),
    ];
    if is_async_generator {
        params.push(make_dp_tuple(vec![
            py_expr!("{name:literal}", name = "/_dp_transport_sent"),
            py_expr!("None"),
            py_expr!("__dp_NO_DEFAULT"),
        ]));
    }
    make_dp_tuple(params)
}

pub(crate) fn stmt_body_from_stmts(stmts: Vec<Stmt>) -> StmtBody {
    StmtBody {
        body: stmts.into_iter().map(Box::new).collect(),
        range: TextRange::default(),
        node_index: ast::AtomicNodeIndex::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::{BbExpr, BbFunction, BbOp, BindingTarget};
    use crate::basic_block::bb_ir::BbBlock;
    use crate::basic_block::bb_ir::{BbGeneratorClosureInit, BbGeneratorClosureSlot, BbTerm};
    use crate::{
        py_expr, transform::Options, transform_str_to_bb_ir_with_options,
        transform_str_to_ruff_with_options,
    };

    fn function_by_name<'a>(bb_module: &'a super::BbModule, bind_name: &str) -> &'a BbFunction {
        let direct = bb_module
            .functions
            .iter()
            .find(|func| func.bind_name == bind_name)
            .unwrap_or_else(|| panic!("missing lowered function {bind_name}; got {:?}", bb_module));
        if direct.generator_closure_layout.is_some() {
            return direct;
        }
        bb_module
            .functions
            .iter()
            .find(|func| func.bind_name == format!("{bind_name}_resume"))
            .unwrap_or(direct)
    }

    fn slot_by_name<'a>(
        slots: &'a [BbGeneratorClosureSlot],
        logical_name: &str,
    ) -> &'a BbGeneratorClosureSlot {
        slots
            .iter()
            .find(|slot| slot.logical_name == logical_name)
            .unwrap_or_else(|| panic!("missing closure slot {logical_name}; got {slots:?}"))
    }

    fn expr_text(expr: &BbExpr) -> String {
        crate::ruff_ast_to_string(&expr.to_expr())
    }

    fn block_uses_text(block: &BbBlock, needle: &str) -> bool {
        block.ops.iter().any(|op| match op {
            BbOp::Assign(assign) => expr_text(&assign.value).contains(needle),
            BbOp::Expr(expr) => expr_text(&expr.value).contains(needle),
            BbOp::Delete(delete) => delete
                .targets
                .iter()
                .any(|expr| expr_text(expr).contains(needle)),
        }) || match &block.term {
            BbTerm::BrIf { test, .. } => expr_text(&test).contains(needle),
            BbTerm::BrTable { index, .. } => expr_text(&index).contains(needle),
            BbTerm::Raise { exc, cause } => {
                exc.as_ref()
                    .is_some_and(|value| expr_text(value).contains(needle))
                    || cause
                        .as_ref()
                        .is_some_and(|value| expr_text(value).contains(needle))
            }
            BbTerm::Ret(value) => value
                .as_ref()
                .is_some_and(|ret| expr_text(ret).contains(needle)),
            _ => false,
        }
    }

    #[test]
    fn lowers_simple_if_function_into_basic_blocks() {
        let source = r#"
def foo(a, b):
    c = a + b
    if c > 5:
        print("hi", c)
    else:
        d = b + 1
        print(d)
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let foo = function_by_name(&bb_module, "foo");
        assert!(foo.blocks.len() >= 3, "{foo:?}");
        assert!(
            foo.blocks
                .iter()
                .any(|block| matches!(block.term, BbTerm::BrIf { .. })),
            "{foo:?}"
        );
    }

    #[test]
    fn exposes_bb_ir_for_lowered_functions() {
        let source = r#"
def foo(a, b):
    if a:
        return b
    return a
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let foo = bb_module
            .functions
            .iter()
            .find(|func| func.bind_name == "foo")
            .expect("foo should be lowered");
        assert!(foo.entry.starts_with("_dp_bb_"), "{:?}", foo.entry);
        assert!(!foo.blocks.is_empty());
    }

    #[test]
    fn nested_global_function_def_lowers_as_module_global() {
        let source = r#"
def build_qualnames():
    def global_function():
        def inner_function():
            global inner_global_function
            def inner_global_function():
                pass
            return inner_global_function
        return inner_function()
    return global_function()
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let inner_global_function = function_by_name(&bb_module, "inner_global_function");
        assert_eq!(
            inner_global_function.binding_target,
            BindingTarget::ModuleGlobal,
            "{inner_global_function:?}"
        );
        assert_eq!(inner_global_function.qualname, "inner_global_function");
    }

    #[test]
    fn closure_backed_generator_does_not_lift_module_globals() {
        let source = r#"
def child():
    yield "start"

def delegator():
    result = yield from child()
    return ("done", result)
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let delegator = function_by_name(&bb_module, "delegator");
        let layout = delegator
            .generator_closure_layout
            .as_ref()
            .expect("closure-backed generator should record closure layout");
        assert!(
            !layout
                .lifted_locals
                .iter()
                .any(|slot| slot.logical_name == "child"),
            "{layout:?}"
        );
        assert!(
            !delegator.entry_params.iter().any(|name| name == "child"),
            "{delegator:?}"
        );
    }

    #[test]
    fn closure_backed_generator_records_explicit_closure_layout() {
        let source = r#"
def outer(scale):
    factor = scale
    def gen(a):
        total = a
        yield total + factor
        total = total + 1
        yield total
    return gen
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let gen = function_by_name(&bb_module, "gen");
        let layout = gen
            .generator_closure_layout
            .as_ref()
            .expect("sync generator should record closure layout");

        let factor = slot_by_name(&layout.inherited_captures, "factor");
        assert_eq!(factor.storage_name, "_dp_cell_factor");
        assert_eq!(factor.init, BbGeneratorClosureInit::InheritedCapture);

        let a = slot_by_name(&layout.lifted_locals, "a");
        assert_eq!(a.storage_name, "_dp_cell_a");
        assert_eq!(a.init, BbGeneratorClosureInit::Parameter);

        let total = slot_by_name(&layout.lifted_locals, "total");
        assert_eq!(total.storage_name, "_dp_cell_total");
        assert_eq!(total.init, BbGeneratorClosureInit::Deferred);

        let pc = slot_by_name(&layout.runtime_cells, "_dp_pc");
        assert_eq!(pc.storage_name, "_dp_cell__dp_pc");
        assert_eq!(pc.init, BbGeneratorClosureInit::RuntimePcZero);
    }

    #[test]
    fn closure_backed_generator_layout_marks_try_exception_slots_deleted() {
        let source = r#"
def gen():
    try:
        yield 1
    except ValueError:
        return 2
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let gen = function_by_name(&bb_module, "gen");
        let layout = gen
            .generator_closure_layout
            .as_ref()
            .expect("sync generator should record closure layout");

        assert!(
            layout
                .lifted_locals
                .iter()
                .any(|slot| slot.init == BbGeneratorClosureInit::DeletedSentinel),
            "{layout:?}"
        );
    }

    #[test]
    fn closure_backed_coroutine_records_explicit_closure_layout() {
        let source = r#"
class Once:
    def __await__(self):
        yield 1
        return 2

def outer(scale):
    factor = scale
    async def run():
        total = 1
        total += factor
        total += await Once()
        return total
    return run
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let run = function_by_name(&bb_module, "run");
        let layout = run
            .generator_closure_layout
            .as_ref()
            .expect("closure-backed coroutine should record closure layout");

        let factor = slot_by_name(&layout.inherited_captures, "factor");
        assert_eq!(factor.storage_name, "_dp_cell_factor");
        assert_eq!(factor.init, BbGeneratorClosureInit::InheritedCapture);

        let total = slot_by_name(&layout.lifted_locals, "total");
        assert_eq!(total.storage_name, "_dp_cell_total");

        let pc = slot_by_name(&layout.runtime_cells, "_dp_pc");
        assert_eq!(pc.storage_name, "_dp_cell__dp_pc");
        assert_eq!(pc.init, BbGeneratorClosureInit::RuntimePcZero);
    }

    #[test]
    fn closure_backed_async_generator_records_explicit_closure_layout() {
        let source = r#"
def outer(scale):
    factor = scale
    async def agen():
        total = 1
        yield total + factor
        total += 1
        yield total + factor
    return agen
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let agen = function_by_name(&bb_module, "agen");
        let layout = agen
            .generator_closure_layout
            .as_ref()
            .expect("closure-backed async generator should record closure layout");

        let factor = slot_by_name(&layout.inherited_captures, "factor");
        assert_eq!(factor.storage_name, "_dp_cell_factor");
        assert_eq!(factor.init, BbGeneratorClosureInit::InheritedCapture);

        let total = slot_by_name(&layout.lifted_locals, "total");
        assert_eq!(total.storage_name, "_dp_cell_total");

        let pc = slot_by_name(&layout.runtime_cells, "_dp_pc");
        assert_eq!(pc.storage_name, "_dp_cell__dp_pc");
        assert_eq!(pc.init, BbGeneratorClosureInit::RuntimePcZero);
    }

    #[test]
    fn lowers_while_break_continue_into_basic_blocks() {
        let source = r#"
def run(limit):
    i = 0
    out = []
    while i < limit:
        i = i + 1
        if i == 2:
            continue
        if i == 5:
            break
        out.append(i)
    else:
        out.append(99)
    return out, i
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let run = function_by_name(&bb_module, "run");
        assert!(
            run.blocks
                .iter()
                .any(|block| matches!(block.term, BbTerm::BrIf { .. })),
            "{run:?}"
        );
        assert!(
            run.blocks
                .iter()
                .any(|block| matches!(block.term, BbTerm::Jump(_))),
            "{run:?}"
        );
    }

    #[test]
    fn lowers_for_else_break_into_basic_blocks() {
        let source = r#"
def run(items):
    out = []
    for x in items:
        if x == 2:
            break
        out.append(x)
    else:
        out.append(99)
    return out
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let run = function_by_name(&bb_module, "run");
        assert!(
            run.blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_next_or_sentinel")),
            "{run:?}"
        );
        assert!(
            run.blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_iter")),
            "{run:?}"
        );
        assert!(
            run.blocks
                .iter()
                .any(|block| matches!(block.term, BbTerm::BrIf { .. })),
            "{run:?}"
        );
    }

    #[test]
    fn lowers_async_for_else_directly_without_completed_flag() {
        let source = r#"
async def run():
    async for x in ait:
        body()
    else:
        done()
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let run = function_by_name(&bb_module, "run");
        let debug = format!("{run:?}");
        assert!(
            run.blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_anext_or_sentinel")),
            "{run:?}"
        );
        assert!(
            run.blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_aiter")),
            "{run:?}"
        );
        assert!(!debug.contains("_dp_completed_"), "{debug}");
    }

    #[test]
    fn omits_synthetic_end_block_when_unreachable() {
        let source = r#"
def f():
    return 1
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let f = function_by_name(&bb_module, "f");
        assert!(f.entry == "_dp_bb_f_start", "{f:?}");
        assert!(
            !f.blocks.iter().any(|block| block.label == "_dp_bb_f_0"),
            "{f:?}"
        );
    }

    #[test]
    fn folds_jump_to_trivial_none_return() {
        let source = r#"
def f():
    x = 1
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let f = function_by_name(&bb_module, "f");
        assert!(
            f.blocks
                .iter()
                .any(|block| matches!(block.term, BbTerm::Ret(None))),
            "{f:?}"
        );
        assert!(
            !f.blocks
                .iter()
                .any(|block| matches!(block.term, BbTerm::Jump(_))),
            "{f:?}"
        );
    }

    #[test]
    fn debug_generator_filter_source_order_ir() {
        let pass_source = r#"
class Field:
    def __init__(self, name, *, init, kw_only=False):
        self.name = name
        self.init = init
        self.kw_only = kw_only

def fields_in_init_order(fields):
    return tuple(
        field.name
        for field in fields
        if field.init and not field.kw_only
    )
"#;
        let fail_source = r#"
def fields_in_init_order(fields):
    return tuple(
        field.name
        for field in fields
        if field.init and not field.kw_only
    )

class Field:
    def __init__(self, name, *, init, kw_only=False):
        self.name = name
        self.init = init
        self.kw_only = kw_only
"#;

        for (name, source) in [("pass", pass_source), ("fail", fail_source)] {
            let lowered = transform_str_to_ruff_with_options(source, Options::for_test())
                .expect("transform should succeed");
            let blockpy = lowered.blockpy_module.expect("expected BlockPy module");
            let blockpy_rendered = crate::basic_block::blockpy_module_to_string(&blockpy);
            eprintln!("==== {name} BLOCKPY ====\n{blockpy_rendered}");

            let bb_module = transform_str_to_bb_ir_with_options(source, Options::for_test())
                .expect("transform should succeed")
                .expect("bb module should be available");
            let function_names = bb_module
                .functions
                .iter()
                .map(|func| format!("{} :: {}", func.bind_name, func.qualname))
                .collect::<Vec<_>>();
            eprintln!(
                "==== {name} BB FUNCTIONS ====\n{}",
                function_names.join("\n")
            );
            let gen = bb_module
                .functions
                .iter()
                .find(|func| func.bind_name.contains("_dp_genexpr"))
                .unwrap_or_else(|| panic!("missing genexpr helper in {name}"));
            eprintln!("==== {name} BB {:?} ====\n{gen:#?}", gen.qualname);

            let prepared = crate::basic_block::prepare_bb_module_for_jit(&bb_module)
                .expect("jit prep should succeed");
            let prepared_gen = prepared
                .functions
                .iter()
                .find(|func| func.bind_name.contains("_dp_genexpr"))
                .unwrap_or_else(|| panic!("missing prepared genexpr helper in {name}"));
            for label in ["_dp_bb__dp_genexpr_1_44", "_dp_bb__dp_genexpr_1_45"] {
                if let Some(block) = prepared_gen
                    .blocks
                    .iter()
                    .find(|block| block.label == label)
                {
                    eprintln!("==== {name} PREPARED {label} ====\n{block:#?}");
                }
            }
        }
    }

    #[test]
    fn closure_backed_simple_generator_records_one_resume_per_yield() {
        let source = r#"
def make_counter(delta):
    outer_capture = delta
    def gen():
        total = 1
        total += outer_capture
        sent = yield total
        total += sent
        yield total
    return gen()
"#;

        let lowered = transform_str_to_ruff_with_options(source, Options::for_test())
            .expect("transform should succeed");
        let blockpy = lowered.blockpy_module.expect("expected BlockPy module");
        let blockpy_gen = blockpy
            .functions
            .iter()
            .find(|func| func.bind_name == "gen")
            .expect("expected lowered gen function");
        let generator_info = blockpy_gen
            .generator
            .as_ref()
            .expect("generator function should record generator metadata");
        assert_eq!(
            generator_info.yield_sites.len(),
            2,
            "{:?}",
            generator_info.yield_sites
        );
        assert_eq!(
            generator_info.resume_order.len(),
            3,
            "{:?}",
            generator_info.resume_order
        );

        let bb_module = transform_str_to_bb_ir_with_options(source, Options::for_test())
            .expect("transform should succeed")
            .expect("bb module should be available");
        let gen = function_by_name(&bb_module, "gen");
        let super::BbFunctionKind::Generator { resume_pcs, .. } = &gen.kind else {
            panic!("expected generator kind, got {:?}", gen.kind);
        };
        assert_eq!(resume_pcs.len(), 3, "{resume_pcs:?}");
    }

    #[test]
    fn lowers_outer_with_nested_nonlocal_inner() {
        let source = r#"
def outer():
    x = 5
    def inner():
        nonlocal x
        x = 2
        return x
    return inner()
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let outer = function_by_name(&bb_module, "outer");
        let inner = function_by_name(&bb_module, "inner");
        assert!(outer.entry == "_dp_bb_outer_start", "{outer:?}");
        assert!(inner.entry == "_dp_bb_inner_start", "{inner:?}");
        assert!(
            outer
                .blocks
                .iter()
                .any(|block| block_uses_text(block, "_dp_cell_x")),
            "{outer:?}"
        );
    }

    #[test]
    fn lowers_try_finally_with_return_via_dispatch() {
        let source = r#"
def f(x):
    try:
        if x:
            return 1
    finally:
        cleanup()
    return 2
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let f = function_by_name(&bb_module, "f");
        assert!(
            f.blocks
                .iter()
                .any(|block| block.exc_target_label.is_some()),
            "{f:?}"
        );
        let debug = format!("{f:?}");
        assert!(!debug.contains("finally:"), "{debug}");
    }

    #[test]
    fn lowers_nested_with_cleanup_and_inner_return_without_hanging() {
        let source = r#"
from pathlib import Path
import tempfile

def run():
    with tempfile.TemporaryDirectory() as temp_dir:
        path = Path(temp_dir) / "example.txt"
        with open(path, "w", encoding="utf8") as handle:
            handle.write("payload")
        with open(path, "r", encoding="utf8") as handle:
            return "ok"
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let run = function_by_name(&bb_module, "run");
        assert!(
            run.blocks
                .iter()
                .any(|block| block.exc_target_label.is_some()),
            "{run:?}"
        );
    }

    #[test]
    fn lowers_plain_try_except_with_try_jump_dispatch() {
        let source = r#"
try:
    print(1)
except Exception:
    print(2)
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let module_init = bb_module
            .module_init
            .as_ref()
            .expect("module init should be present");
        let init_fn = function_by_name(&bb_module, module_init);
        assert!(
            init_fn
                .blocks
                .iter()
                .any(|block| block.exc_target_label.is_some()),
            "{init_fn:?}"
        );
    }

    #[test]
    fn lowers_try_star_except_star_via_exceptiongroup_split() {
        let source = r#"
def f():
    try:
        raise ExceptionGroup("eg", [ValueError(1)])
    except* ValueError as exc:
        return exc
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let f = function_by_name(&bb_module, "f");
        assert!(
            f.blocks
                .iter()
                .any(|block| block_uses_text(block, "__dp_exceptiongroup_split")),
            "{f:?}"
        );
        assert!(
            f.blocks
                .iter()
                .any(|block| block.exc_target_label.is_some()),
            "{f:?}"
        );
    }

    #[test]
    fn dead_tail_local_binding_still_raises_unbound() {
        let source = r#"
def f():
    print(x)
    return
    x = 1
"#;

        let options = Options::for_test();
        let bb_module = transform_str_to_bb_ir_with_options(source, options)
            .expect("transform should succeed")
            .expect("bb module should be available");
        let f = function_by_name(&bb_module, "f");
        let debug = format!("{f:?}");
        assert!(debug.contains("load_deleted_name"), "{debug}");
        assert!(debug.contains("DELETED"), "{debug}");
        assert!(!debug.contains("x = 1"), "{debug}");
    }

    #[test]
    fn matches_dp_lookup_call_with_decoded_name_arg() {
        let expr =
            py_expr!("__dp_getattr(__dp__, __dp_decode_literal_bytes(b\"current_exception\"))");
        assert!(super::lowering_helpers::is_dp_lookup_call(
            &expr,
            "current_exception",
        ));
    }
}
