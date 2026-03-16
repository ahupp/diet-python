mod codegen_normalize;
mod codegen_trace;
mod exception_pass;

use super::annotation_export::build_exec_function_def_binding_stmts;
use super::bb_ir::{BbBlock, BbBlockMeta, BbFunction, BbModule, BbStmt, BbTerm};
use super::block_py::cfg::linearize_structured_ifs;
use super::block_py::state::collect_parameter_names;
use super::block_py::{
    BlockPyBlock, BlockPyIfTerm, BlockPyStmt, BlockPyTerm, CoreBlockPyCall, CoreBlockPyCallArg,
    CoreBlockPyCallableDef, CoreBlockPyCallableDefWithoutAwait,
    CoreBlockPyCallableDefWithoutAwaitOrYield, CoreBlockPyExprWithoutAwaitOrYield,
    CoreBlockPyKeywordArg, CoreBlockPyLiteral, CoreBlockPyStmtWithoutAwaitOrYield,
};
use super::blockpy_expr_simplify::simplify_blockpy_callable_def_exprs;
use super::cfg_ir::{CfgCallableDef, CfgModule};
use super::function_lowering::rewrite_deleted_name_loads;
use super::lowered_ir::BindingTarget;
use super::ruff_to_blockpy::{
    build_lowered_blockpy_function_export_plan,
    lower_awaits_in_lowered_blockpy_function_bundle_plan,
    lower_generators_in_lowered_blockpy_function_bundle_plan,
    lowered_blockpy_function_export_plan_to_bundle, LoweredBlockPyFunction,
    LoweredBlockPyFunctionBundlePlan, LoweredBlockPyFunctionExportPlan,
    ResolvedLoweredBlockPyFunctionBundlePlan,
};
use crate::basic_block::ast_to_ast::context::Context;
use ruff_python_ast::{self as ast};
use ruff_text_size::TextRange;
use std::collections::{HashMap, HashSet};

pub use codegen_normalize::normalize_bb_module_for_codegen;
pub use exception_pass::lower_try_jump_exception_flow;

pub type LoweredBlockPyModuleBundle = CfgModule<LoweredBlockPyFunction>;
pub type LoweredCoreBlockPyFunction = LoweredBlockPyFunction<CoreBlockPyCallableDef>;
pub type LoweredCoreBlockPyFunctionWithoutAwait =
    LoweredBlockPyFunction<CoreBlockPyCallableDefWithoutAwait>;
pub type LoweredCoreBlockPyFunctionWithoutAwaitOrYield =
    LoweredBlockPyFunction<CoreBlockPyCallableDefWithoutAwaitOrYield>;

pub type LoweredCoreBlockPyModuleBundle = CfgModule<LoweredCoreBlockPyFunction>;
pub type LoweredCoreBlockPyModuleBundleWithoutAwait =
    CfgModule<LoweredCoreBlockPyFunctionWithoutAwait>;
pub type LoweredCoreBlockPyModuleBundleWithoutAwaitOrYield =
    CfgModule<LoweredCoreBlockPyFunctionWithoutAwaitOrYield>;

#[derive(Clone)]
pub(crate) struct LoweredBlockPyModuleBundlePlanEntry {
    pub bundle_plan: LoweredBlockPyFunctionBundlePlan,
    pub main_binding_target: BindingTarget,
}

#[derive(Clone)]
pub(crate) struct LoweredBlockPyModuleBundlePlan {
    pub module_init: Option<String>,
    pub callable_def_bundles: Vec<LoweredBlockPyModuleBundlePlanEntry>,
    pub next_block_id: usize,
    pub next_function_id: usize,
}

#[derive(Clone)]
pub(crate) struct ResolvedLoweredBlockPyModuleBundlePlanEntry {
    pub bundle_plan: ResolvedLoweredBlockPyFunctionBundlePlan,
    pub main_binding_target: BindingTarget,
}

#[derive(Clone)]
pub(crate) struct ResolvedLoweredBlockPyModuleBundlePlan {
    pub module_init: Option<String>,
    pub callable_def_bundles: Vec<ResolvedLoweredBlockPyModuleBundlePlanEntry>,
}

#[derive(Clone)]
pub(crate) struct LoweredBlockPyModuleExportPlanEntry {
    pub bundle_plan: LoweredBlockPyFunctionExportPlan,
    pub main_binding_target: BindingTarget,
}

#[derive(Clone)]
pub(crate) struct LoweredBlockPyModuleExportPlan {
    pub module_init: Option<String>,
    pub callable_def_bundles: Vec<LoweredBlockPyModuleExportPlanEntry>,
}

fn next_temp_from_reserved_names(
    reserved_names: &mut HashSet<String>,
    prefix: &str,
    next_id: &mut usize,
) -> String {
    loop {
        let current = *next_id;
        *next_id += 1;
        let candidate = format!("_dp_{prefix}_{current}");
        if reserved_names.contains(candidate.as_str()) {
            continue;
        }
        reserved_names.insert(candidate.clone());
        return candidate;
    }
}

pub(crate) fn lower_awaits_in_lowered_blockpy_module_bundle_plan(
    context: &Context,
    plan: LoweredBlockPyModuleBundlePlan,
) -> LoweredBlockPyModuleBundlePlan {
    LoweredBlockPyModuleBundlePlan {
        module_init: plan.module_init,
        callable_def_bundles: plan
            .callable_def_bundles
            .into_iter()
            .map(|entry| LoweredBlockPyModuleBundlePlanEntry {
                bundle_plan: lower_awaits_in_lowered_blockpy_function_bundle_plan(
                    context,
                    entry.bundle_plan,
                ),
                main_binding_target: entry.main_binding_target,
            })
            .collect(),
        next_block_id: plan.next_block_id,
        next_function_id: plan.next_function_id,
    }
}

pub(crate) fn lower_generators_in_lowered_blockpy_module_bundle_plan(
    context: &Context,
    plan: LoweredBlockPyModuleBundlePlan,
) -> ResolvedLoweredBlockPyModuleBundlePlan {
    let LoweredBlockPyModuleBundlePlan {
        module_init,
        callable_def_bundles,
        mut next_block_id,
        mut next_function_id,
    } = plan;
    let mut resolved_entries = Vec::with_capacity(callable_def_bundles.len());
    for entry in callable_def_bundles {
        let cell_slots = entry.bundle_plan.cell_slots.clone();
        let outer_scope_names = entry.bundle_plan.outer_scope_names.clone();
        let mut reserved_temp_names = outer_scope_names.clone();
        let bundle_plan = lower_generators_in_lowered_blockpy_function_bundle_plan(
            context,
            entry.bundle_plan,
            &mut next_block_id,
            &mut next_function_id,
            &mut |func_def| {
                build_exec_function_def_binding_stmts(func_def, &cell_slots, &outer_scope_names)
            },
            &mut |prefix, next_block_id| {
                next_temp_from_reserved_names(&mut reserved_temp_names, prefix, next_block_id)
            },
        );
        resolved_entries.push(ResolvedLoweredBlockPyModuleBundlePlanEntry {
            bundle_plan,
            main_binding_target: entry.main_binding_target,
        });
    }
    ResolvedLoweredBlockPyModuleBundlePlan {
        module_init,
        callable_def_bundles: resolved_entries,
    }
}

fn build_lowered_blockpy_function_export_plan_with_deleted_name_rewrite(
    plan: ResolvedLoweredBlockPyFunctionBundlePlan,
) -> LoweredBlockPyFunctionExportPlan {
    let deleted_names = plan.deleted_names.clone();
    let unbound_local_names = plan.unbound_local_names.clone();
    build_lowered_blockpy_function_export_plan(plan, &mut |callable_def| {
        if !deleted_names.is_empty() {
            rewrite_deleted_name_loads(
                &mut callable_def.blocks,
                &deleted_names,
                &unbound_local_names,
            );
        } else if !unbound_local_names.is_empty() {
            rewrite_deleted_name_loads(
                &mut callable_def.blocks,
                &HashSet::new(),
                &unbound_local_names,
            );
        }
    })
}

pub(crate) fn resolved_lowered_blockpy_module_bundle_plan_to_export_plan(
    plan: ResolvedLoweredBlockPyModuleBundlePlan,
) -> LoweredBlockPyModuleExportPlan {
    let mut callable_def_bundles = Vec::new();
    for entry in plan.callable_def_bundles {
        let bundle_plan =
            build_lowered_blockpy_function_export_plan_with_deleted_name_rewrite(entry.bundle_plan);
        callable_def_bundles.push(LoweredBlockPyModuleExportPlanEntry {
            bundle_plan,
            main_binding_target: entry.main_binding_target,
        });
    }
    LoweredBlockPyModuleExportPlan {
        module_init: plan.module_init,
        callable_def_bundles,
    }
}

pub(crate) fn lower_yield_in_lowered_blockpy_module_export_plan(
    plan: LoweredBlockPyModuleExportPlan,
) -> LoweredBlockPyModuleBundle {
    let mut callable_defs = Vec::new();
    for entry in plan.callable_def_bundles {
        let bundle = lowered_blockpy_function_export_plan_to_bundle(entry.bundle_plan);
        callable_defs.extend(
            bundle
                .helper_functions
                .into_iter()
                .map(|helper| helper.with_binding_target(BindingTarget::Local)),
        );
        callable_defs.push(
            bundle
                .main_function
                .with_binding_target(entry.main_binding_target),
        );
    }
    LoweredBlockPyModuleBundle {
        module_init: plan.module_init,
        callable_defs,
    }
}

pub(crate) fn lower_blockpy_module_plan_to_bundle(
    context: &Context,
    plan: LoweredBlockPyModuleBundlePlan,
) -> LoweredBlockPyModuleBundle {
    let await_free = lower_awaits_in_lowered_blockpy_module_bundle_plan(context, plan);
    let generator_lowered =
        lower_generators_in_lowered_blockpy_module_bundle_plan(context, await_free);
    let export_plan = resolved_lowered_blockpy_module_bundle_plan_to_export_plan(generator_lowered);
    lower_yield_in_lowered_blockpy_module_export_plan(export_plan)
}

pub fn project_lowered_module_callable_defs<T, U: Clone>(
    module: &CfgModule<T>,
    project: impl Fn(&T) -> &U,
) -> CfgModule<U> {
    module.map_callable_defs(|lowered_function| project(lowered_function).clone())
}

pub(crate) fn simplify_lowered_blockpy_module_bundle_exprs(
    module: &LoweredBlockPyModuleBundle,
) -> LoweredCoreBlockPyModuleBundle {
    module.map_callable_defs(simplify_lowered_blockpy_function_exprs)
}

fn lower_core_callable_def_without_await(
    callable_def: &CoreBlockPyCallableDef,
) -> CoreBlockPyCallableDefWithoutAwait {
    let qualname = callable_def.qualname.as_str();
    callable_def.clone().try_into().unwrap_or_else(|_| {
        panic!(
            "core BlockPy await lowering is not explicit yet: await reached the core no-await boundary for {}",
            qualname
        )
    })
}

fn lower_core_blockpy_function_without_await(
    lowered: &LoweredCoreBlockPyFunction,
) -> LoweredCoreBlockPyFunctionWithoutAwait {
    LoweredCoreBlockPyFunctionWithoutAwait {
        binding_target: lowered.binding_target,
        callable_def: lower_core_callable_def_without_await(&lowered.callable_def),
        is_coroutine: lowered.is_coroutine,
        bb_kind: lowered.bb_kind.clone(),
        block_params: lowered.block_params.clone(),
        exception_edges: lowered.exception_edges.clone(),
        closure_layout: lowered.closure_layout.clone(),
    }
}

fn lower_core_callable_def_without_await_or_yield(
    callable_def: &CoreBlockPyCallableDefWithoutAwait,
) -> CoreBlockPyCallableDefWithoutAwaitOrYield {
    let qualname = callable_def.qualname.as_str();
    callable_def.clone().try_into().unwrap_or_else(|_| {
        panic!(
            "core BlockPy yield lowering is not explicit yet: yield-family expr reached the core no-yield boundary for {}",
            qualname
        )
    })
}

fn lower_core_blockpy_function_without_await_or_yield(
    lowered: &LoweredCoreBlockPyFunctionWithoutAwait,
) -> LoweredCoreBlockPyFunctionWithoutAwaitOrYield {
    LoweredCoreBlockPyFunctionWithoutAwaitOrYield {
        binding_target: lowered.binding_target,
        callable_def: lower_core_callable_def_without_await_or_yield(&lowered.callable_def),
        is_coroutine: lowered.is_coroutine,
        bb_kind: lowered.bb_kind.clone(),
        block_params: lowered.block_params.clone(),
        exception_edges: lowered.exception_edges.clone(),
        closure_layout: lowered.closure_layout.clone(),
    }
}

pub(crate) fn lower_awaits_in_lowered_core_blockpy_module_bundle(
    module: LoweredCoreBlockPyModuleBundle,
) -> LoweredCoreBlockPyModuleBundleWithoutAwait {
    module.map_callable_defs(lower_core_blockpy_function_without_await)
}

pub(crate) fn lower_yield_in_lowered_core_blockpy_module_bundle(
    module: LoweredCoreBlockPyModuleBundleWithoutAwait,
) -> LoweredCoreBlockPyModuleBundleWithoutAwaitOrYield {
    module.map_callable_defs(lower_core_blockpy_function_without_await_or_yield)
}

pub(crate) fn lower_core_blockpy_module_bundle_to_bb_module(
    module: &LoweredCoreBlockPyModuleBundleWithoutAwaitOrYield,
) -> BbModule {
    module.map_callable_defs(lower_core_blockpy_function_to_bb_function)
}

fn simplify_lowered_blockpy_function_exprs(
    lowered: &LoweredBlockPyFunction,
) -> LoweredCoreBlockPyFunction {
    let callable_def = simplify_blockpy_callable_def_exprs(&lowered.callable_def);
    LoweredCoreBlockPyFunction {
        binding_target: lowered.binding_target,
        callable_def,
        is_coroutine: lowered.is_coroutine,
        bb_kind: lowered.bb_kind.clone(),
        block_params: lowered.block_params.clone(),
        exception_edges: lowered.exception_edges.clone(),
        closure_layout: lowered.closure_layout.clone(),
    }
}

pub(crate) fn lower_core_blockpy_function_to_bb_function(
    lowered: &LoweredCoreBlockPyFunctionWithoutAwaitOrYield,
) -> BbFunction {
    let (linear_blocks, linear_block_params, linear_exception_edges) = linearize_structured_ifs(
        &lowered.callable_def.blocks,
        &lowered.block_params,
        &lowered.exception_edges,
    );
    BbFunction {
        cfg: CfgCallableDef {
            function_id: lowered.callable_def.function_id,
            bind_name: lowered.callable_def.bind_name.clone(),
            display_name: lowered.callable_def.display_name.clone(),
            qualname: lowered.callable_def.qualname.clone(),
            kind: lowered.bb_kind.clone(),
            params: collect_parameter_names(&lowered.callable_def.params),
            entry_liveins: lowered.callable_def.entry_liveins.clone(),
            blocks: lower_blockpy_blocks_to_bb_blocks(
                &linear_blocks,
                &linear_block_params,
                &linear_exception_edges,
            ),
        },
        binding_target: lowered.binding_target,
        closure_layout: lowered.closure_layout.clone(),
        local_cell_slots: lowered.callable_def.local_cell_slots.clone(),
    }
}

pub(crate) fn lower_blockpy_blocks_to_bb_blocks(
    blocks: &[BlockPyBlock<CoreBlockPyExprWithoutAwaitOrYield>],
    block_params: &HashMap<String, Vec<String>>,
    exception_edges: &HashMap<String, Option<String>>,
) -> Vec<BbBlock> {
    let block_exc_params = blocks
        .iter()
        .map(|block| {
            (
                block.label.as_str().to_string(),
                block.meta.exc_param.clone(),
            )
        })
        .collect::<HashMap<_, _>>();
    blocks
        .iter()
        .map(|block| {
            let current_exception_name = block.meta.exc_param.as_deref();
            let mut normalized_body = block.body.clone();
            if let Some(exc_name) = current_exception_name {
                for stmt in &mut normalized_body {
                    rewrite_current_exception_in_blockpy_stmt(stmt, exc_name);
                }
            }
            let mut normalized_term = block.term.clone();
            if let Some(exc_name) = current_exception_name {
                rewrite_current_exception_in_blockpy_term(&mut normalized_term, exc_name);
            }
            let exc_target_label = exception_edges.get(block.label.as_str()).cloned().flatten();
            let exc_name = exc_target_label.as_ref().and_then(|target_label| {
                block_exc_params
                    .get(target_label.as_str())
                    .cloned()
                    .flatten()
                    .or_else(|| {
                        block_params
                            .get(target_label.as_str())
                            .and_then(|params| exception_param_from_block_params(params))
                    })
            });
            let ops = normalized_body
                .into_iter()
                .map(bb_stmt_from_blockpy_stmt)
                .collect::<Vec<_>>();
            let mut params = block_params
                .get(block.label.as_str())
                .cloned()
                .unwrap_or_default();
            if let Some(exc_param) = block.meta.exc_param.as_ref() {
                if !params.iter().any(|param| param == exc_param) {
                    params.push(exc_param.clone());
                }
            }
            BbBlock {
                label: block.label.as_str().to_string(),
                body: ops,
                term: bb_term_from_blockpy_term(&normalized_term),
                meta: BbBlockMeta {
                    params,
                    exc_target_label,
                    exc_name,
                },
            }
        })
        .collect()
}

fn exception_param_from_block_params(params: &[String]) -> Option<String> {
    params.iter().find_map(|name| {
        (name.starts_with("_dp_try_exc_") || name.starts_with("_dp_uncaught_exc_"))
            .then(|| name.clone())
    })
}

fn rewrite_current_exception_in_blockpy_stmt(
    stmt: &mut CoreBlockPyStmtWithoutAwaitOrYield,
    exc_name: &str,
) {
    match stmt {
        BlockPyStmt::Assign(assign) => {
            rewrite_current_exception_in_blockpy_expr(&mut assign.value, exc_name);
        }
        BlockPyStmt::Expr(expr) => {
            rewrite_current_exception_in_blockpy_expr(expr, exc_name);
        }
        BlockPyStmt::Delete(_) => {}
        BlockPyStmt::If(if_stmt) => {
            rewrite_current_exception_in_blockpy_expr(&mut if_stmt.test, exc_name);
            for stmt in &mut if_stmt.body.body {
                rewrite_current_exception_in_blockpy_stmt(stmt, exc_name);
            }
            if let Some(term) = if_stmt.body.term.as_mut() {
                rewrite_current_exception_in_blockpy_term(term, exc_name);
            }
            for stmt in &mut if_stmt.orelse.body {
                rewrite_current_exception_in_blockpy_stmt(stmt, exc_name);
            }
            if let Some(term) = if_stmt.orelse.term.as_mut() {
                rewrite_current_exception_in_blockpy_term(term, exc_name);
            }
        }
    }
}

fn rewrite_current_exception_in_blockpy_term(
    term: &mut BlockPyTerm<CoreBlockPyExprWithoutAwaitOrYield>,
    exc_name: &str,
) {
    match term {
        BlockPyTerm::IfTerm(BlockPyIfTerm { test, .. }) => {
            rewrite_current_exception_in_blockpy_expr(test, exc_name);
        }
        BlockPyTerm::BranchTable(branch) => {
            rewrite_current_exception_in_blockpy_expr(&mut branch.index, exc_name);
        }
        BlockPyTerm::Raise(raise_stmt) => {
            if let Some(exc) = raise_stmt.exc.as_mut() {
                rewrite_current_exception_in_blockpy_expr(exc, exc_name);
            } else {
                raise_stmt.exc = Some(current_exception_name_expr(exc_name).into());
            }
        }
        BlockPyTerm::Return(value) => {
            if let Some(value) = value.as_mut() {
                rewrite_current_exception_in_blockpy_expr(value, exc_name);
            }
        }
        BlockPyTerm::Jump(_) | BlockPyTerm::TryJump(_) => {}
    }
}

fn rewrite_current_exception_in_blockpy_expr(
    expr: &mut CoreBlockPyExprWithoutAwaitOrYield,
    exc_name: &str,
) {
    if let CoreBlockPyExprWithoutAwaitOrYield::Call(call) = expr {
        rewrite_current_exception_in_blockpy_expr(call.func.as_mut(), exc_name);
        for arg in &mut call.args {
            match arg {
                CoreBlockPyCallArg::Positional(value) | CoreBlockPyCallArg::Starred(value) => {
                    rewrite_current_exception_in_blockpy_expr(value, exc_name);
                }
            }
        }
        for keyword in &mut call.keywords {
            match keyword {
                CoreBlockPyKeywordArg::Named { value, .. }
                | CoreBlockPyKeywordArg::Starred(value) => {
                    rewrite_current_exception_in_blockpy_expr(value, exc_name);
                }
            }
        }
    }

    if is_current_exception_call(expr) {
        *expr = current_exception_name_expr(exc_name);
    } else if is_exc_info_call(expr) {
        *expr = current_exception_info_expr(exc_name);
    }
}

fn is_current_exception_call(expr: &CoreBlockPyExprWithoutAwaitOrYield) -> bool {
    let CoreBlockPyExprWithoutAwaitOrYield::Call(call) = expr else {
        return false;
    };
    call.args.is_empty()
        && call.keywords.is_empty()
        && is_dp_lookup_call_expr(call.func.as_ref(), "current_exception")
}

fn is_exc_info_call(expr: &CoreBlockPyExprWithoutAwaitOrYield) -> bool {
    let CoreBlockPyExprWithoutAwaitOrYield::Call(call) = expr else {
        return false;
    };
    call.args.is_empty()
        && call.keywords.is_empty()
        && is_dp_lookup_call_expr(call.func.as_ref(), "exc_info")
}

fn is_dp_lookup_call_expr(func: &CoreBlockPyExprWithoutAwaitOrYield, attr_name: &str) -> bool {
    match func {
        CoreBlockPyExprWithoutAwaitOrYield::Name(name) => {
            name.id.as_str() == format!("__dp_{attr_name}")
        }
        CoreBlockPyExprWithoutAwaitOrYield::Call(call)
            if call.keywords.is_empty() && call.args.len() == 2 =>
        {
            matches!(
                call.func.as_ref(),
                CoreBlockPyExprWithoutAwaitOrYield::Name(name)
                    if name.id.as_str() == "__dp_getattr"
            ) && matches!(
                &call.args[0],
                CoreBlockPyCallArg::Positional(CoreBlockPyExprWithoutAwaitOrYield::Name(base))
                    if base.id.as_str() == "__dp__"
            ) && expr_static_str(match &call.args[1] {
                CoreBlockPyCallArg::Positional(value) => value,
                CoreBlockPyCallArg::Starred(_) => return false,
            }) == Some(attr_name.to_string())
        }
        _ => false,
    }
}

fn expr_static_str(expr: &CoreBlockPyExprWithoutAwaitOrYield) -> Option<String> {
    match expr {
        CoreBlockPyExprWithoutAwaitOrYield::Literal(CoreBlockPyLiteral::StringLiteral(value)) => {
            Some(value.value.to_str().to_string())
        }
        CoreBlockPyExprWithoutAwaitOrYield::Literal(CoreBlockPyLiteral::BytesLiteral(bytes)) => {
            let value: std::borrow::Cow<[u8]> = (&bytes.value).into();
            String::from_utf8(value.into_owned()).ok()
        }
        CoreBlockPyExprWithoutAwaitOrYield::Call(call)
            if call.keywords.is_empty()
                && call.args.len() == 1
                && matches!(
                    call.func.as_ref(),
                    CoreBlockPyExprWithoutAwaitOrYield::Name(name)
                        if matches!(
                            name.id.as_str(),
                            "__dp_decode_literal_bytes" | "__dp_decode_literal_source_bytes"
                        )
                ) =>
        {
            match &call.args[0] {
                CoreBlockPyCallArg::Positional(CoreBlockPyExprWithoutAwaitOrYield::Literal(
                    CoreBlockPyLiteral::BytesLiteral(bytes),
                )) => {
                    let value: std::borrow::Cow<[u8]> = (&bytes.value).into();
                    String::from_utf8(value.into_owned()).ok()
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn current_exception_name_expr(exc_name: &str) -> CoreBlockPyExprWithoutAwaitOrYield {
    CoreBlockPyExprWithoutAwaitOrYield::Name(ast::ExprName {
        id: exc_name.into(),
        ctx: ast::ExprContext::Load,
        range: compat_range(),
        node_index: compat_node_index(),
    })
}

fn current_exception_info_expr(exc_name: &str) -> CoreBlockPyExprWithoutAwaitOrYield {
    CoreBlockPyExprWithoutAwaitOrYield::Call(CoreBlockPyCall {
        node_index: compat_node_index(),
        range: compat_range(),
        func: Box::new(CoreBlockPyExprWithoutAwaitOrYield::Name(ast::ExprName {
            id: "__dp_exc_info_from_exception".into(),
            ctx: ast::ExprContext::Load,
            range: compat_range(),
            node_index: compat_node_index(),
        })),
        args: vec![CoreBlockPyCallArg::Positional(current_exception_name_expr(
            exc_name,
        ))],
        keywords: Vec::new(),
    })
}

fn compat_node_index() -> ast::AtomicNodeIndex {
    ast::AtomicNodeIndex::default()
}

fn compat_range() -> TextRange {
    TextRange::default()
}

fn bb_stmt_from_blockpy_stmt(stmt: CoreBlockPyStmtWithoutAwaitOrYield) -> BbStmt {
    match stmt {
        BlockPyStmt::Assign(_) | BlockPyStmt::Expr(_) | BlockPyStmt::Delete(_) => stmt,
        BlockPyStmt::If(_) => {
            panic!("structured BlockPy If reached BB block body after linearization")
        }
    }
}

fn bb_term_from_blockpy_term(terminal: &BlockPyTerm<CoreBlockPyExprWithoutAwaitOrYield>) -> BbTerm {
    match terminal {
        BlockPyTerm::Jump(target) => BbTerm::Jump(target.as_str().to_string()),
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test,
            then_label,
            else_label,
        }) => BbTerm::BrIf {
            test: test.clone(),
            then_label: then_label.as_str().to_string(),
            else_label: else_label.as_str().to_string(),
        },
        BlockPyTerm::BranchTable(branch) => BbTerm::BrTable {
            index: branch.index.clone(),
            targets: branch
                .targets
                .iter()
                .map(|label| label.as_str().to_string())
                .collect(),
            default_label: branch.default_label.as_str().to_string(),
        },
        BlockPyTerm::Raise(raise_stmt) => BbTerm::Raise {
            exc: raise_stmt.exc.clone(),
            cause: None,
        },
        BlockPyTerm::TryJump(try_jump) => BbTerm::Jump(try_jump.body_label.as_str().to_string()),
        BlockPyTerm::Return(value) => BbTerm::Ret(value.clone()),
    }
}

#[cfg(test)]
mod tests {
    use crate::basic_block::bb_ir::BbTerm;
    use crate::basic_block::block_py::cfg::linearize_structured_ifs;
    use crate::basic_block::block_py::{
        BlockPyAssign, BlockPyBlock, BlockPyIf, BlockPyIfTerm, BlockPyLabel, BlockPyStmt,
        BlockPyStmtFragment, BlockPyTerm, CoreBlockPyCall, CoreBlockPyCallArg,
        CoreBlockPyExprWithoutAwaitOrYield,
    };
    use crate::basic_block::blockpy_to_bb::lower_blockpy_blocks_to_bb_blocks;
    use crate::py_expr;
    use ruff_python_ast::{self as ast, Expr};
    use ruff_text_size::TextRange;
    use std::collections::HashMap;

    fn name_expr(name: &str) -> ruff_python_ast::ExprName {
        let Expr::Name(name_expr) = py_expr!("{name:id}", name = name) else {
            unreachable!();
        };
        name_expr
    }

    #[test]
    fn linearizes_structured_if_stmt_into_explicit_blocks() {
        let block: BlockPyBlock<Expr> = BlockPyBlock {
            label: BlockPyLabel::from("start"),
            body: vec![
                BlockPyStmt::Assign(BlockPyAssign {
                    target: name_expr("x"),
                    value: py_expr!("a").into(),
                }),
                BlockPyStmt::If(BlockPyIf {
                    test: py_expr!("cond").into(),
                    body: BlockPyStmtFragment::from_stmts(vec![BlockPyStmt::Assign(
                        BlockPyAssign {
                            target: name_expr("x"),
                            value: py_expr!("b").into(),
                        },
                    )]),
                    orelse: BlockPyStmtFragment::from_stmts(vec![BlockPyStmt::Assign(
                        BlockPyAssign {
                            target: name_expr("x"),
                            value: py_expr!("c").into(),
                        },
                    )]),
                }),
                BlockPyStmt::Expr(py_expr!("sink(x)").into()),
            ],
            term: BlockPyTerm::Return(None),
            meta: Default::default(),
        };

        let (blocks, _, _): (
            Vec<BlockPyBlock<Expr>>,
            HashMap<String, Vec<String>>,
            HashMap<String, Option<String>>,
        ) = linearize_structured_ifs(&[block], &HashMap::new(), &HashMap::new());

        assert_eq!(blocks.len(), 4, "{blocks:?}");
        assert!(matches!(
            blocks[0].term,
            BlockPyTerm::IfTerm(BlockPyIfTerm { .. })
        ));
        assert!(!blocks.iter().any(|block| {
            block
                .body
                .iter()
                .any(|stmt| matches!(stmt, BlockPyStmt::If(_)))
        }));
    }

    fn core_name_expr(name: &str) -> CoreBlockPyExprWithoutAwaitOrYield {
        CoreBlockPyExprWithoutAwaitOrYield::Name(ast::ExprName {
            id: name.into(),
            ctx: ast::ExprContext::Load,
            range: TextRange::default(),
            node_index: ast::AtomicNodeIndex::default(),
        })
    }

    fn core_call_expr(
        name: &str,
        args: Vec<CoreBlockPyExprWithoutAwaitOrYield>,
    ) -> CoreBlockPyExprWithoutAwaitOrYield {
        CoreBlockPyExprWithoutAwaitOrYield::Call(CoreBlockPyCall {
            node_index: ast::AtomicNodeIndex::default(),
            range: TextRange::default(),
            func: Box::new(core_name_expr(name)),
            args: args
                .into_iter()
                .map(CoreBlockPyCallArg::Positional)
                .collect(),
            keywords: Vec::new(),
        })
    }

    #[test]
    fn rewrites_current_exception_placeholders_in_final_core_blocks() {
        let block = BlockPyBlock {
            label: BlockPyLabel::from("start"),
            body: vec![BlockPyStmt::Expr(core_call_expr(
                "__dp_current_exception",
                Vec::new(),
            ))],
            term: BlockPyTerm::Return(Some(core_call_expr("__dp_exc_info", Vec::new()))),
            meta: crate::basic_block::block_py::BlockPyBlockMeta {
                exc_param: Some("_dp_try_exc_0".to_string()),
            },
        };

        let lowered = lower_blockpy_blocks_to_bb_blocks(&[block], &HashMap::new(), &HashMap::new());
        let block = &lowered[0];

        let BlockPyStmt::Expr(body_expr) = &block.body[0] else {
            panic!("expected expr stmt in lowered BB block");
        };
        assert!(matches!(
            body_expr,
            CoreBlockPyExprWithoutAwaitOrYield::Name(name) if name.id.as_str() == "_dp_try_exc_0"
        ));

        let BbTerm::Ret(Some(CoreBlockPyExprWithoutAwaitOrYield::Call(call))) = &block.term else {
            panic!("expected rewritten return expr");
        };
        assert!(matches!(
            call.func.as_ref(),
            CoreBlockPyExprWithoutAwaitOrYield::Name(name)
                if name.id.as_str() == "__dp_exc_info_from_exception"
        ));
        assert!(matches!(
            call.args.as_slice(),
            [CoreBlockPyCallArg::Positional(CoreBlockPyExprWithoutAwaitOrYield::Name(name))]
                if name.id.as_str() == "_dp_try_exc_0"
        ));
    }
}
