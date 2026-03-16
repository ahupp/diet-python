mod codegen_normalize;
mod codegen_trace;
mod exception_pass;

use super::annotation_export::build_exec_function_def_binding_stmts;
use super::bb_ir::{BbBlock, BbBlockMeta, BbFunction, BbModule, BbOp, BbTerm};
use super::block_py::cfg::linearize_structured_ifs;
use super::block_py::exception::is_dp_lookup_call;
use super::block_py::state::collect_parameter_names;
use super::block_py::{
    BlockPyBlock, BlockPyIfTerm, BlockPyStmt, BlockPyTerm, CoreBlockPyCallableDef,
    CoreBlockPyCallableDefWithoutAwait, CoreBlockPyCallableDefWithoutAwaitOrYield, CoreBlockPyExpr,
    CoreBlockPyExprWithoutAwait, CoreBlockPyExprWithoutAwaitOrYield,
};
use super::blockpy_expr_simplify::simplify_blockpy_callable_def_exprs;
use super::cfg_ir::{CfgCallableDef, CfgModule};
use super::function_lowering::rewrite_deleted_name_loads;
use super::param_specs::function_param_specs_expr;
use super::ruff_to_blockpy::{
    build_lowered_blockpy_function_export_plan,
    lower_awaits_in_lowered_blockpy_function_bundle_plan,
    lower_generators_in_lowered_blockpy_function_bundle_plan,
    lowered_blockpy_function_export_plan_to_bundle, LoweredBlockPyFunction,
    LoweredBlockPyFunctionBundlePlan, LoweredBlockPyFunctionExportPlan,
    ResolvedLoweredBlockPyFunctionBundlePlan,
};
use crate::basic_block::ast_to_ast::context::Context;
use crate::py_expr;
use crate::transformer::{walk_expr, Transformer};
use ruff_python_ast::{self as ast, Expr};
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
    pub main_binding_target: super::bb_ir::BindingTarget,
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
    pub main_binding_target: super::bb_ir::BindingTarget,
}

#[derive(Clone)]
pub(crate) struct ResolvedLoweredBlockPyModuleBundlePlan {
    pub module_init: Option<String>,
    pub callable_def_bundles: Vec<ResolvedLoweredBlockPyModuleBundlePlanEntry>,
}

#[derive(Clone)]
pub(crate) struct LoweredBlockPyModuleExportPlanEntry {
    pub bundle_plan: LoweredBlockPyFunctionExportPlan,
    pub main_binding_target: super::bb_ir::BindingTarget,
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
                .map(|helper| helper.with_binding_target(super::bb_ir::BindingTarget::Local)),
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

fn lower_core_expr_without_await(
    expr: &CoreBlockPyExpr,
    qualname: &str,
) -> CoreBlockPyExprWithoutAwait {
    expr.clone().try_into().unwrap_or_else(|_| {
        panic!(
            "core BlockPy await lowering is not explicit yet: await reached the core no-await boundary for {}",
            qualname
        )
    })
}

fn lower_core_stmt_without_await(
    stmt: &BlockPyStmt<CoreBlockPyExpr>,
    qualname: &str,
) -> BlockPyStmt<CoreBlockPyExprWithoutAwait> {
    match stmt {
        BlockPyStmt::Assign(assign) => BlockPyStmt::Assign(super::block_py::BlockPyAssign {
            target: assign.target.clone(),
            value: lower_core_expr_without_await(&assign.value, qualname),
        }),
        BlockPyStmt::Expr(expr) => BlockPyStmt::Expr(lower_core_expr_without_await(expr, qualname)),
        BlockPyStmt::Delete(delete) => BlockPyStmt::Delete(delete.clone()),
        BlockPyStmt::If(if_stmt) => BlockPyStmt::If(super::block_py::BlockPyIf {
            test: lower_core_expr_without_await(&if_stmt.test, qualname),
            body: lower_core_fragment_without_await(&if_stmt.body, qualname),
            orelse: lower_core_fragment_without_await(&if_stmt.orelse, qualname),
        }),
    }
}

fn lower_core_term_without_await(
    term: &BlockPyTerm<CoreBlockPyExpr>,
    qualname: &str,
) -> BlockPyTerm<CoreBlockPyExprWithoutAwait> {
    match term {
        BlockPyTerm::Jump(target) => BlockPyTerm::Jump(target.clone()),
        BlockPyTerm::IfTerm(if_term) => BlockPyTerm::IfTerm(super::block_py::BlockPyIfTerm {
            test: lower_core_expr_without_await(&if_term.test, qualname),
            then_label: if_term.then_label.clone(),
            else_label: if_term.else_label.clone(),
        }),
        BlockPyTerm::BranchTable(branch) => {
            BlockPyTerm::BranchTable(super::block_py::BlockPyBranchTable {
                index: lower_core_expr_without_await(&branch.index, qualname),
                targets: branch.targets.clone(),
                default_label: branch.default_label.clone(),
            })
        }
        BlockPyTerm::Raise(raise_stmt) => BlockPyTerm::Raise(super::block_py::BlockPyRaise {
            exc: raise_stmt
                .exc
                .as_ref()
                .map(|exc| lower_core_expr_without_await(exc, qualname)),
        }),
        BlockPyTerm::TryJump(try_jump) => BlockPyTerm::TryJump(try_jump.clone()),
        BlockPyTerm::Return(value) => BlockPyTerm::Return(
            value
                .as_ref()
                .map(|value| lower_core_expr_without_await(value, qualname)),
        ),
    }
}

fn lower_core_fragment_without_await(
    fragment: &super::block_py::BlockPyCfgFragment<
        super::block_py::BlockPyStmt<CoreBlockPyExpr>,
        super::block_py::BlockPyTerm<CoreBlockPyExpr>,
    >,
    qualname: &str,
) -> super::block_py::BlockPyCfgFragment<
    super::block_py::BlockPyStmt<CoreBlockPyExprWithoutAwait>,
    super::block_py::BlockPyTerm<CoreBlockPyExprWithoutAwait>,
> {
    super::block_py::BlockPyCfgFragment::with_term(
        fragment
            .body
            .iter()
            .map(|stmt| lower_core_stmt_without_await(stmt, qualname))
            .collect(),
        fragment
            .term
            .as_ref()
            .map(|term| lower_core_term_without_await(term, qualname)),
    )
}

fn lower_core_callable_def_without_await(
    callable_def: &CoreBlockPyCallableDef,
) -> CoreBlockPyCallableDefWithoutAwait {
    let qualname = callable_def.qualname.as_str();
    super::block_py::BlockPyCallableDef {
        cfg: CfgCallableDef {
            function_id: callable_def.function_id,
            bind_name: callable_def.bind_name.clone(),
            display_name: callable_def.display_name.clone(),
            qualname: callable_def.qualname.clone(),
            kind: callable_def.kind,
            params: callable_def.params.clone(),
            entry_liveins: callable_def.entry_liveins.clone(),
            blocks: callable_def
                .blocks
                .iter()
                .map(|block| BlockPyBlock {
                    label: block.label.clone(),
                    body: block
                        .body
                        .iter()
                        .map(|stmt| lower_core_stmt_without_await(stmt, qualname))
                        .collect(),
                    term: lower_core_term_without_await(&block.term, qualname),
                    meta: block.meta.clone(),
                })
                .collect(),
        },
        doc: callable_def
            .doc
            .as_ref()
            .map(|doc| lower_core_expr_without_await(doc, qualname)),
        closure_layout: callable_def.closure_layout.clone(),
        local_cell_slots: callable_def.local_cell_slots.clone(),
    }
}

fn lower_core_blockpy_function_without_await(
    lowered: &LoweredCoreBlockPyFunction,
) -> LoweredCoreBlockPyFunctionWithoutAwait {
    LoweredCoreBlockPyFunctionWithoutAwait {
        binding_target: lowered.binding_target,
        param_specs: lowered.param_specs.clone(),
        callable_def: lower_core_callable_def_without_await(&lowered.callable_def),
        is_coroutine: lowered.is_coroutine,
        bb_kind: lowered.bb_kind.clone(),
        block_params: lowered.block_params.clone(),
        exception_edges: lowered.exception_edges.clone(),
        closure_layout: lowered.closure_layout.clone(),
    }
}

fn lower_core_expr_without_await_or_yield(
    expr: &CoreBlockPyExprWithoutAwait,
    qualname: &str,
) -> CoreBlockPyExprWithoutAwaitOrYield {
    expr.clone().try_into().unwrap_or_else(|_| {
        panic!(
            "core BlockPy yield lowering is not explicit yet: yield-family expr reached the core no-yield boundary for {}",
            qualname
        )
    })
}

fn lower_core_stmt_without_await_or_yield(
    stmt: &BlockPyStmt<CoreBlockPyExprWithoutAwait>,
    qualname: &str,
) -> BlockPyStmt<CoreBlockPyExprWithoutAwaitOrYield> {
    match stmt {
        BlockPyStmt::Assign(assign) => BlockPyStmt::Assign(super::block_py::BlockPyAssign {
            target: assign.target.clone(),
            value: lower_core_expr_without_await_or_yield(&assign.value, qualname),
        }),
        BlockPyStmt::Expr(expr) => {
            BlockPyStmt::Expr(lower_core_expr_without_await_or_yield(expr, qualname))
        }
        BlockPyStmt::Delete(delete) => BlockPyStmt::Delete(delete.clone()),
        BlockPyStmt::If(if_stmt) => BlockPyStmt::If(super::block_py::BlockPyIf {
            test: lower_core_expr_without_await_or_yield(&if_stmt.test, qualname),
            body: lower_core_fragment_without_await_or_yield(&if_stmt.body, qualname),
            orelse: lower_core_fragment_without_await_or_yield(&if_stmt.orelse, qualname),
        }),
    }
}

fn lower_core_term_without_await_or_yield(
    term: &BlockPyTerm<CoreBlockPyExprWithoutAwait>,
    qualname: &str,
) -> BlockPyTerm<CoreBlockPyExprWithoutAwaitOrYield> {
    match term {
        BlockPyTerm::Jump(target) => BlockPyTerm::Jump(target.clone()),
        BlockPyTerm::IfTerm(if_term) => BlockPyTerm::IfTerm(super::block_py::BlockPyIfTerm {
            test: lower_core_expr_without_await_or_yield(&if_term.test, qualname),
            then_label: if_term.then_label.clone(),
            else_label: if_term.else_label.clone(),
        }),
        BlockPyTerm::BranchTable(branch) => {
            BlockPyTerm::BranchTable(super::block_py::BlockPyBranchTable {
                index: lower_core_expr_without_await_or_yield(&branch.index, qualname),
                targets: branch.targets.clone(),
                default_label: branch.default_label.clone(),
            })
        }
        BlockPyTerm::Raise(raise_stmt) => BlockPyTerm::Raise(super::block_py::BlockPyRaise {
            exc: raise_stmt
                .exc
                .as_ref()
                .map(|exc| lower_core_expr_without_await_or_yield(exc, qualname)),
        }),
        BlockPyTerm::TryJump(try_jump) => BlockPyTerm::TryJump(try_jump.clone()),
        BlockPyTerm::Return(value) => BlockPyTerm::Return(
            value
                .as_ref()
                .map(|value| lower_core_expr_without_await_or_yield(value, qualname)),
        ),
    }
}

fn lower_core_fragment_without_await_or_yield(
    fragment: &super::block_py::BlockPyCfgFragment<
        super::block_py::BlockPyStmt<CoreBlockPyExprWithoutAwait>,
        super::block_py::BlockPyTerm<CoreBlockPyExprWithoutAwait>,
    >,
    qualname: &str,
) -> super::block_py::BlockPyCfgFragment<
    super::block_py::BlockPyStmt<CoreBlockPyExprWithoutAwaitOrYield>,
    super::block_py::BlockPyTerm<CoreBlockPyExprWithoutAwaitOrYield>,
> {
    super::block_py::BlockPyCfgFragment::with_term(
        fragment
            .body
            .iter()
            .map(|stmt| lower_core_stmt_without_await_or_yield(stmt, qualname))
            .collect(),
        fragment
            .term
            .as_ref()
            .map(|term| lower_core_term_without_await_or_yield(term, qualname)),
    )
}

fn lower_core_callable_def_without_await_or_yield(
    callable_def: &CoreBlockPyCallableDefWithoutAwait,
) -> CoreBlockPyCallableDefWithoutAwaitOrYield {
    let qualname = callable_def.qualname.as_str();
    super::block_py::BlockPyCallableDef {
        cfg: CfgCallableDef {
            function_id: callable_def.function_id,
            bind_name: callable_def.bind_name.clone(),
            display_name: callable_def.display_name.clone(),
            qualname: callable_def.qualname.clone(),
            kind: callable_def.kind,
            params: callable_def.params.clone(),
            entry_liveins: callable_def.entry_liveins.clone(),
            blocks: callable_def
                .blocks
                .iter()
                .map(|block| BlockPyBlock {
                    label: block.label.clone(),
                    body: block
                        .body
                        .iter()
                        .map(|stmt| lower_core_stmt_without_await_or_yield(stmt, qualname))
                        .collect(),
                    term: lower_core_term_without_await_or_yield(&block.term, qualname),
                    meta: block.meta.clone(),
                })
                .collect(),
        },
        doc: callable_def
            .doc
            .as_ref()
            .map(|doc| lower_core_expr_without_await_or_yield(doc, qualname)),
        closure_layout: callable_def.closure_layout.clone(),
        local_cell_slots: callable_def.local_cell_slots.clone(),
    }
}

fn lower_core_blockpy_function_without_await_or_yield(
    lowered: &LoweredCoreBlockPyFunctionWithoutAwait,
) -> LoweredCoreBlockPyFunctionWithoutAwaitOrYield {
    LoweredCoreBlockPyFunctionWithoutAwaitOrYield {
        binding_target: lowered.binding_target,
        param_specs: lowered.param_specs.clone(),
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
        param_specs: CoreBlockPyExprWithoutAwaitOrYield::from_expr(function_param_specs_expr(
            &callable_def.params,
        )),
        callable_def,
        is_coroutine: lowered.is_coroutine,
        bb_kind: lowered.bb_kind.clone(),
        block_params: lowered.block_params.clone(),
        exception_edges: lowered.exception_edges.clone(),
        closure_layout: lowered.closure_layout.clone(),
    }
}

pub(crate) fn lower_core_blockpy_function_to_bb_function<E>(
    lowered: &LoweredBlockPyFunction<super::block_py::BlockPyCallableDef<E>>,
) -> BbFunction
where
    E: Clone + Into<Expr> + From<Expr>,
{
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
        is_coroutine: lowered.is_coroutine,
        closure_layout: lowered.closure_layout.clone(),
        local_cell_slots: lowered.callable_def.local_cell_slots.clone(),
    }
}

pub(crate) fn lower_blockpy_blocks_to_bb_blocks(
    blocks: &[BlockPyBlock<impl Clone + Into<Expr> + From<Expr>>],
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
                .map(bb_op_from_blockpy_stmt)
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
                    local_defs: Vec::new(),
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

fn rewrite_current_exception_in_blockpy_stmt<E>(stmt: &mut BlockPyStmt<E>, exc_name: &str)
where
    E: Clone + Into<Expr> + From<Expr>,
{
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

fn rewrite_current_exception_in_blockpy_term<E>(term: &mut BlockPyTerm<E>, exc_name: &str)
where
    E: Clone + Into<Expr> + From<Expr>,
{
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

fn rewrite_current_exception_in_blockpy_expr<E>(expr: &mut E, exc_name: &str)
where
    E: Clone + Into<Expr> + From<Expr>,
{
    let mut raw: Expr = expr.clone().into();
    rewrite_current_exception_in_expr(&mut raw, exc_name);
    *expr = raw.into();
}

fn rewrite_current_exception_in_expr(expr: &mut Expr, exc_name: &str) {
    CurrentExceptionTransformer { exc_name }.visit_expr(expr);
}

struct CurrentExceptionTransformer<'a> {
    exc_name: &'a str,
}

impl Transformer for CurrentExceptionTransformer<'_> {
    fn visit_expr(&mut self, expr: &mut Expr) {
        walk_expr(self, expr);
        if is_current_exception_call(expr) {
            *expr = current_exception_name_expr(self.exc_name);
        } else if is_exc_info_call(expr) {
            *expr = current_exception_info_expr(self.exc_name);
        }
    }
}

fn is_current_exception_call(expr: &Expr) -> bool {
    let Expr::Call(call) = expr else {
        return false;
    };
    call.arguments.args.is_empty()
        && call.arguments.keywords.is_empty()
        && is_dp_lookup_call(call.func.as_ref(), "current_exception")
}

fn is_exc_info_call(expr: &Expr) -> bool {
    let Expr::Call(call) = expr else {
        return false;
    };
    call.arguments.args.is_empty()
        && call.arguments.keywords.is_empty()
        && is_dp_lookup_call(call.func.as_ref(), "exc_info")
}

fn current_exception_name_expr(exc_name: &str) -> Expr {
    Expr::Name(ast::ExprName {
        id: exc_name.into(),
        ctx: ast::ExprContext::Load,
        range: compat_range(),
        node_index: compat_node_index(),
    })
}

fn current_exception_info_expr(exc_name: &str) -> Expr {
    py_expr!(
        "__dp_exc_info_from_exception({exc:expr})",
        exc = current_exception_name_expr(exc_name),
    )
}

fn compat_node_index() -> ast::AtomicNodeIndex {
    ast::AtomicNodeIndex::default()
}

fn compat_range() -> TextRange {
    TextRange::default()
}

fn bb_op_from_blockpy_stmt<E>(stmt: BlockPyStmt<E>) -> BbOp
where
    E: Clone + Into<Expr>,
{
    match stmt {
        BlockPyStmt::Assign(assign) => BbOp::Assign(super::bb_ir::BbAssignOp {
            node_index: compat_node_index(),
            range: compat_range(),
            target: assign.target,
            value: CoreBlockPyExprWithoutAwaitOrYield::from_expr(assign.value.into()),
        }),
        BlockPyStmt::Expr(expr) => BbOp::Expr(super::bb_ir::BbExprOp {
            node_index: compat_node_index(),
            range: compat_range(),
            value: CoreBlockPyExprWithoutAwaitOrYield::from_expr(expr.into()),
        }),
        BlockPyStmt::Delete(delete) => BbOp::Delete(super::bb_ir::BbDeleteOp {
            node_index: compat_node_index(),
            range: compat_range(),
            targets: vec![CoreBlockPyExprWithoutAwaitOrYield::Name(delete.target)],
        }),
        BlockPyStmt::If(_) => {
            panic!("structured BlockPy If reached BB block body after linearization")
        }
    }
}

fn bb_term_from_blockpy_term<E>(terminal: &BlockPyTerm<E>) -> BbTerm
where
    E: Clone + Into<Expr>,
{
    match terminal {
        BlockPyTerm::Jump(target) => BbTerm::Jump(target.as_str().to_string()),
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test,
            then_label,
            else_label,
        }) => BbTerm::BrIf {
            test: CoreBlockPyExprWithoutAwaitOrYield::from_expr(test.clone().into()),
            then_label: then_label.as_str().to_string(),
            else_label: else_label.as_str().to_string(),
        },
        BlockPyTerm::BranchTable(branch) => BbTerm::BrTable {
            index: CoreBlockPyExprWithoutAwaitOrYield::from_expr(branch.index.clone().into()),
            targets: branch
                .targets
                .iter()
                .map(|label| label.as_str().to_string())
                .collect(),
            default_label: branch.default_label.as_str().to_string(),
        },
        BlockPyTerm::Raise(raise_stmt) => BbTerm::Raise {
            exc: raise_stmt
                .exc
                .as_ref()
                .map(|exc| CoreBlockPyExprWithoutAwaitOrYield::from_expr(exc.clone().into())),
            cause: None,
        },
        BlockPyTerm::TryJump(try_jump) => BbTerm::Jump(try_jump.body_label.as_str().to_string()),
        BlockPyTerm::Return(value) => BbTerm::Ret(
            value
                .clone()
                .map(|expr| CoreBlockPyExprWithoutAwaitOrYield::from_expr(expr.into())),
        ),
    }
}

#[cfg(test)]
mod tests {
    use crate::basic_block::block_py::cfg::linearize_structured_ifs;
    use crate::basic_block::block_py::{
        BlockPyAssign, BlockPyBlock, BlockPyIf, BlockPyIfTerm, BlockPyLabel, BlockPyStmt,
        BlockPyStmtFragment, BlockPyTerm,
    };
    use crate::py_expr;
    use ruff_python_ast::Expr;
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
}
