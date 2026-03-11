use super::super::bb_ir::{BbFunctionKind, BindingTarget};
use super::super::blockpy_to_bb::{
    push_lowered_blockpy_function_bundle, LoweredBlockPyModuleBundle,
};
use super::super::function_identity::{resolve_runtime_function_identity, FunctionIdentity};
use super::super::function_lowering::{
    function_docstring_expr, try_lower_function_to_blockpy_bundle,
};
use super::super::ruff_to_blockpy::LoweredBlockPyFunction;
use super::dataflow::analyze_blockpy_use_def;
use super::state::collect_parameter_names;
use crate::basic_block::annotation_export::{
    build_lowered_annotation_helper_binding, is_annotation_helper_name,
    prepare_non_lowered_annotationlib_function,
};
use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::ast_to_ast::rewrite_stmt;
use crate::basic_block::ast_to_ast::scope::{cell_name, is_internal_symbol, Scope};
use crate::template::into_body;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, name::Name, Expr, NodeIndex, Stmt};
use ruff_python_parser::parse_expression;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

pub(crate) enum LoweredFunctionPlacementPlan {
    ReplaceWith(Stmt),
    HoistToParent {
        replacement: Stmt,
        hoisted_to_parent: Vec<Stmt>,
    },
}

pub(crate) enum NonLoweredFunctionPlacementPlan {
    ReplaceWith(Stmt),
    PrependBody(Vec<Stmt>),
    LeaveInPlace,
}

pub(crate) struct LoweredFunctionBindingPlan {
    pub target: BindingTarget,
    pub needs_cell_sync: bool,
}

pub(crate) struct LoweredFunctionExportPlan {
    pub identity: FunctionIdentity,
    pub binding: LoweredFunctionBindingPlan,
}

pub(crate) struct LoweredFunctionRewriteResult {
    pub replacement: Stmt,
}

pub(crate) struct LoweredFunctionVisitPlan {
    pub binding_target: BindingTarget,
    pub rewrite: LoweredFunctionRewriteResult,
}

pub(crate) enum NonLoweredFunctionBindingPlan {
    LeaveLocal,
    CellSyncOnly,
    Rebind { target: BindingTarget },
}

pub(crate) enum NonLoweredLocalNamePlan {
    KeepOriginal,
    UseFreshTemp,
}

pub(crate) struct NonLoweredFunctionExportPlan {
    pub identity: FunctionIdentity,
    pub binding: NonLoweredFunctionBindingPlan,
    pub local_name_plan: Option<NonLoweredLocalNamePlan>,
}

pub(crate) fn build_def_expr_from_lowered(
    lowered: &LoweredBlockPyFunction,
    doc_expr: Option<Expr>,
    annotate_fn_expr: Option<Expr>,
) -> Option<Expr> {
    let entry_label = lowered.entry_label.as_str();
    let entry_ref_expr = py_expr!("{entry:literal}", entry = entry_label);
    let param_names: HashSet<String> = collect_parameter_names(&lowered.function.params)
        .into_iter()
        .collect();
    let generator_lifted_state_names: HashSet<&str> = lowered
        .closure_layout
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
    let generator_closure_storage_names: HashSet<&str> = lowered
        .closure_layout
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
    let locally_assigned: HashSet<String> = lowered
        .function
        .blocks
        .iter()
        .flat_map(|block| analyze_blockpy_use_def(block).1.into_iter())
        .collect();
    let mut closure_items = Vec::new();
    for entry_name in &lowered.entry_params {
        if param_names.contains(entry_name) {
            closure_items.push(py_expr!("{value:literal}", value = entry_name.as_str(),));
        } else if entry_name == "_dp_classcell"
            || (entry_name.starts_with("_dp_cell_")
                && !lowered.local_cell_slots.contains(entry_name))
        {
            let value = name_expr(entry_name.as_str())?;
            closure_items.push(make_dp_tuple(vec![
                py_expr!("{value:literal}", value = entry_name.as_str()),
                value,
            ]));
        } else if matches!(
            &lowered.bb_kind,
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
            &lowered.bb_kind,
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
        } else if !entry_name.starts_with("_dp_") && !locally_assigned.contains(entry_name) {
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
        name = lowered.display_name.as_str(),
        qualname = lowered.function.qualname.as_str(),
        closure = closure.clone(),
        params = lowered.param_specs.to_expr(),
        module_globals = py_expr!("__dp_globals()"),
        module_name = py_expr!("__name__"),
        doc = doc.clone(),
        annotate_fn = annotate_fn.clone(),
    );
    match &lowered.bb_kind {
        BbFunctionKind::Function => {
            if lowered.is_coroutine {
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
                name = lowered.display_name.as_str(),
                qualname = lowered.function.qualname.as_str(),
                closure = closure,
                params = lowered.param_specs.to_expr(),
                doc = doc.clone(),
                annotate_fn = annotate_fn.clone(),
            ))
        }
        BbFunctionKind::Generator { closure_state, .. } => {
            if *closure_state {
                if lowered.is_coroutine {
                    return Some(py_expr!(
                        "__dp_mark_coroutine_function({func:expr})",
                        func = function_entry_expr,
                    ));
                }
                return Some(function_entry_expr);
            }
            if lowered.is_coroutine {
                Some(py_expr!(
                    "__dp_def_coro_from_gen({resume:expr}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, __dp_globals(), __name__, {doc:expr}, {annotate_fn:expr})",
                    resume = entry_ref_expr,
                    name = lowered.display_name.as_str(),
                    qualname = lowered.function.qualname.as_str(),
                    closure = closure,
                    params = lowered.param_specs.to_expr(),
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

pub(crate) fn build_binding_stmt(target: BindingTarget, bind_name: &str, value: Expr) -> Stmt {
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

pub(crate) fn build_cell_sync_stmt(bind_name: &str) -> Stmt {
    let cell = cell_name(bind_name);
    py_stmt!(
        "__dp_store_cell({cell:id}, {name:id})",
        cell = cell.as_str(),
        name = bind_name,
    )
}

fn resolve_function_binding_target(
    binding_target: BindingTarget,
    bind_name: &str,
    qualname: &str,
) -> BindingTarget {
    if binding_target == BindingTarget::Local
        && qualname == bind_name
        && !is_internal_symbol(bind_name)
    {
        BindingTarget::ModuleGlobal
    } else {
        binding_target
    }
}

pub(crate) fn plan_lowered_function_binding(
    binding_target: BindingTarget,
    bind_name: &str,
    qualname: &str,
    needs_cell_sync: bool,
) -> LoweredFunctionBindingPlan {
    LoweredFunctionBindingPlan {
        target: resolve_function_binding_target(binding_target, bind_name, qualname),
        needs_cell_sync,
    }
}

pub(crate) fn plan_lowered_function_export(
    func: &ast::StmtFunctionDef,
    function_identity_by_node: &HashMap<NodeIndex, FunctionIdentity>,
    current_parent: Option<&str>,
    needs_cell_sync: bool,
) -> LoweredFunctionExportPlan {
    let identity =
        resolve_runtime_function_identity(func, function_identity_by_node, current_parent);
    let binding = plan_lowered_function_binding(
        identity.binding_target,
        identity.bind_name.as_str(),
        identity.qualname.as_str(),
        needs_cell_sync,
    );
    LoweredFunctionExportPlan { identity, binding }
}

pub(crate) fn plan_non_lowered_function_binding(
    binding_target: BindingTarget,
    bind_name: &str,
    qualname: &str,
    needs_cell_sync: bool,
) -> NonLoweredFunctionBindingPlan {
    match resolve_function_binding_target(binding_target, bind_name, qualname) {
        BindingTarget::Local => {
            if needs_cell_sync {
                NonLoweredFunctionBindingPlan::CellSyncOnly
            } else {
                NonLoweredFunctionBindingPlan::LeaveLocal
            }
        }
        target => NonLoweredFunctionBindingPlan::Rebind { target },
    }
}

pub(crate) fn plan_non_lowered_local_name(
    local_name: &str,
    bind_name: &str,
    is_annotation_helper: bool,
) -> NonLoweredLocalNamePlan {
    if !is_internal_symbol(local_name) && !is_annotation_helper && local_name == bind_name {
        NonLoweredLocalNamePlan::UseFreshTemp
    } else {
        NonLoweredLocalNamePlan::KeepOriginal
    }
}

pub(crate) fn plan_non_lowered_function_export(
    func: &ast::StmtFunctionDef,
    function_identity_by_node: &HashMap<NodeIndex, FunctionIdentity>,
    current_parent: Option<&str>,
    needs_cell_sync: bool,
    is_annotation_helper: bool,
) -> NonLoweredFunctionExportPlan {
    let identity =
        resolve_runtime_function_identity(func, function_identity_by_node, current_parent);
    let binding = plan_non_lowered_function_binding(
        identity.binding_target,
        identity.bind_name.as_str(),
        identity.qualname.as_str(),
        needs_cell_sync,
    );
    let local_name_plan = if matches!(binding, NonLoweredFunctionBindingPlan::Rebind { .. }) {
        Some(plan_non_lowered_local_name(
            func.name.id.as_str(),
            identity.bind_name.as_str(),
            is_annotation_helper,
        ))
    } else {
        None
    };
    NonLoweredFunctionExportPlan {
        identity,
        binding,
        local_name_plan,
    }
}

pub(crate) fn build_updated_function_binding_stmt(
    target: BindingTarget,
    bind_name: &str,
    local_name: &str,
    qualname: &str,
    display_name: &str,
    doc: Expr,
    decorators: Vec<ast::Decorator>,
) -> Stmt {
    let updated = py_expr!(
        "__dp_update_fn({name:id}, {qualname:literal}, {display_name:literal}, {doc:expr})",
        name = local_name,
        qualname = qualname,
        display_name = display_name,
        doc = doc,
    );
    let value = rewrite_stmt::decorator::rewrite(decorators, updated);
    build_binding_stmt(target, bind_name, value)
}

pub(crate) fn build_non_lowered_binding_stmt(
    func: &mut ast::StmtFunctionDef,
    bind_name: &str,
    qualname: &str,
    display_name: &str,
    binding_plan: NonLoweredFunctionBindingPlan,
    fresh_local_name: Option<String>,
    doc: Expr,
) -> Option<Stmt> {
    match binding_plan {
        NonLoweredFunctionBindingPlan::LeaveLocal => None,
        NonLoweredFunctionBindingPlan::CellSyncOnly => Some(build_cell_sync_stmt(bind_name)),
        NonLoweredFunctionBindingPlan::Rebind { target } => {
            let local_name = if let Some(local_name) = fresh_local_name {
                func.name.id = Name::new(local_name.as_str());
                local_name
            } else {
                func.name.id.to_string()
            };
            let decorators = std::mem::take(&mut func.decorator_list);
            Some(build_updated_function_binding_stmt(
                target,
                bind_name,
                local_name.as_str(),
                qualname,
                display_name,
                doc,
                decorators,
            ))
        }
    }
}

pub(crate) fn plan_lowered_function_placement(
    bind_name: &str,
    entering_module_init: bool,
    has_parent_hoisted_scope: bool,
    function_hoisted: Vec<Stmt>,
    binding_stmt: Stmt,
) -> LoweredFunctionPlacementPlan {
    let keep_local_blocks = !entering_module_init
        && has_parent_hoisted_scope
        && (bind_name.starts_with("_dp_class_ns_") || bind_name.starts_with("_dp_define_class_"));

    if entering_module_init || keep_local_blocks || !has_parent_hoisted_scope {
        let mut body = function_hoisted;
        body.push(binding_stmt);
        LoweredFunctionPlacementPlan::ReplaceWith(into_body(body))
    } else {
        LoweredFunctionPlacementPlan::HoistToParent {
            replacement: binding_stmt,
            hoisted_to_parent: function_hoisted,
        }
    }
}

pub(crate) fn plan_non_lowered_function_placement(
    function_hoisted: Vec<Stmt>,
    function_stmt: Stmt,
    binding_stmt: Option<Stmt>,
) -> NonLoweredFunctionPlacementPlan {
    if let Some(binding_stmt) = binding_stmt {
        let mut body = function_hoisted;
        body.push(function_stmt);
        body.push(binding_stmt);
        NonLoweredFunctionPlacementPlan::ReplaceWith(into_body(body))
    } else if !function_hoisted.is_empty() {
        NonLoweredFunctionPlacementPlan::PrependBody(function_hoisted)
    } else {
        NonLoweredFunctionPlacementPlan::LeaveInPlace
    }
}

pub(crate) fn apply_lowered_function_placement(
    parent_hoisted: Option<&mut Vec<Stmt>>,
    plan: LoweredFunctionPlacementPlan,
) -> Stmt {
    match plan {
        LoweredFunctionPlacementPlan::ReplaceWith(replacement) => replacement,
        LoweredFunctionPlacementPlan::HoistToParent {
            replacement,
            mut hoisted_to_parent,
        } => {
            if let Some(parent_hoisted) = parent_hoisted {
                parent_hoisted.append(&mut hoisted_to_parent);
            }
            replacement
        }
    }
}

pub(crate) fn apply_non_lowered_function_placement(
    func: &mut ast::StmtFunctionDef,
    plan: NonLoweredFunctionPlacementPlan,
) -> Option<Stmt> {
    match plan {
        NonLoweredFunctionPlacementPlan::ReplaceWith(replacement) => Some(replacement),
        NonLoweredFunctionPlacementPlan::PrependBody(function_hoisted) => {
            let mut new_body = function_hoisted
                .into_iter()
                .map(Box::new)
                .collect::<Vec<_>>();
            new_body.extend(std::mem::take(&mut func.body.body));
            func.body.body = new_body;
            None
        }
        NonLoweredFunctionPlacementPlan::LeaveInPlace => None,
    }
}

pub(crate) fn build_lowered_binding_stmt(
    func: &ast::StmtFunctionDef,
    lowered: &LoweredBlockPyFunction,
    target: BindingTarget,
    bind_name: &str,
    doc_expr: Option<Expr>,
    needs_cell_sync: bool,
) -> Option<Stmt> {
    let annotate_helper = build_lowered_annotation_helper_binding(func, bind_name);
    let annotate_fn_expr = annotate_helper
        .as_ref()
        .map(|(_, annotate_fn_expr)| annotate_fn_expr.clone());
    let base_expr = build_def_expr_from_lowered(lowered, doc_expr, annotate_fn_expr)?;
    let decorated = rewrite_stmt::decorator::rewrite(func.decorator_list.clone(), base_expr);
    let binding_stmt = build_binding_stmt(target, bind_name, decorated);
    let mut stmts = Vec::new();
    if let Some((helper_stmt, _)) = annotate_helper {
        stmts.push(helper_stmt);
    }
    stmts.push(binding_stmt);
    if target == BindingTarget::Local && needs_cell_sync {
        stmts.push(build_cell_sync_stmt(bind_name));
    }
    if stmts.len() == 1 {
        stmts.into_iter().next()
    } else {
        Some(into_body(stmts))
    }
}

pub(crate) fn rewrite_lowered_function_stmt(
    parent_hoisted: Option<&mut Vec<Stmt>>,
    func: &ast::StmtFunctionDef,
    lowered: &LoweredBlockPyFunction,
    export_plan: &LoweredFunctionExportPlan,
    entering_module_init: bool,
    has_parent_hoisted_scope: bool,
    function_hoisted: Vec<Stmt>,
    doc_expr: Option<Expr>,
) -> Option<LoweredFunctionRewriteResult> {
    let binding_stmt = build_lowered_binding_stmt(
        func,
        lowered,
        export_plan.binding.target,
        export_plan.identity.bind_name.as_str(),
        doc_expr,
        export_plan.binding.needs_cell_sync,
    )?;
    let replacement = apply_lowered_function_placement(
        parent_hoisted,
        plan_lowered_function_placement(
            export_plan.identity.bind_name.as_str(),
            entering_module_init,
            has_parent_hoisted_scope,
            function_hoisted,
            binding_stmt,
        ),
    );
    Some(LoweredFunctionRewriteResult { replacement })
}

pub(crate) fn plan_and_rewrite_lowered_function_stmt(
    parent_hoisted: Option<&mut Vec<Stmt>>,
    func: &ast::StmtFunctionDef,
    lowered: &LoweredBlockPyFunction,
    function_identity_by_node: &HashMap<NodeIndex, FunctionIdentity>,
    current_parent: Option<&str>,
    needs_cell_sync: bool,
    entering_module_init: bool,
    has_parent_hoisted_scope: bool,
    function_hoisted: Vec<Stmt>,
    doc_expr: Option<Expr>,
) -> Option<LoweredFunctionVisitPlan> {
    let export_plan = plan_lowered_function_export(
        func,
        function_identity_by_node,
        current_parent,
        needs_cell_sync,
    );
    let binding_target = export_plan.binding.target;
    let rewrite = rewrite_lowered_function_stmt(
        parent_hoisted,
        func,
        lowered,
        &export_plan,
        entering_module_init,
        has_parent_hoisted_scope,
        function_hoisted,
        doc_expr,
    )?;
    Some(LoweredFunctionVisitPlan {
        binding_target,
        rewrite,
    })
}

pub(crate) fn rewrite_non_lowered_function_stmt(
    func: &mut ast::StmtFunctionDef,
    export_plan: NonLoweredFunctionExportPlan,
    function_hoisted: Vec<Stmt>,
    doc: Expr,
    mut next_temp: impl FnMut() -> String,
) -> Option<Stmt> {
    let fresh_local_name = match export_plan.local_name_plan {
        Some(NonLoweredLocalNamePlan::UseFreshTemp) => Some(next_temp()),
        Some(NonLoweredLocalNamePlan::KeepOriginal) | None => None,
    };
    let binding_stmt = build_non_lowered_binding_stmt(
        func,
        export_plan.identity.bind_name.as_str(),
        export_plan.identity.qualname.as_str(),
        export_plan.identity.display_name.as_str(),
        export_plan.binding,
        fresh_local_name,
        doc,
    );
    apply_non_lowered_function_placement(
        func,
        plan_non_lowered_function_placement(
            function_hoisted,
            Stmt::FunctionDef(func.clone()),
            binding_stmt,
        ),
    )
}

pub(crate) fn plan_and_rewrite_non_lowered_function_stmt(
    context: &Context,
    func: &mut ast::StmtFunctionDef,
    function_identity_by_node: &HashMap<NodeIndex, FunctionIdentity>,
    current_parent: Option<&str>,
    needs_cell_sync: bool,
    function_hoisted: Vec<Stmt>,
    doc: Expr,
    next_temp: impl FnMut() -> String,
) -> Option<Stmt> {
    prepare_non_lowered_annotationlib_function(context, func);
    let export_plan = plan_non_lowered_function_export(
        func,
        function_identity_by_node,
        current_parent,
        needs_cell_sync,
        is_annotation_helper_name(func.name.id.as_str()),
    );
    rewrite_non_lowered_function_stmt(func, export_plan, function_hoisted, doc, next_temp)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn rewrite_function_def_stmt_via_blockpy(
    context: &Context,
    module_scope: &Arc<Scope>,
    lowered_blockpy_module: &mut LoweredBlockPyModuleBundle,
    parent_hoisted: Option<&mut Vec<Stmt>>,
    function_identity_by_node: &HashMap<NodeIndex, FunctionIdentity>,
    func: &mut ast::StmtFunctionDef,
    current_parent: Option<&str>,
    needs_cell_sync: bool,
    entering_module_init: bool,
    has_parent_hoisted_scope: bool,
    function_hoisted: Vec<Stmt>,
    reserved_temp_names_stack: &mut Vec<HashSet<String>>,
    used_label_prefixes: &mut HashMap<String, usize>,
    next_block_id: &mut usize,
) -> Option<Stmt> {
    let doc_expr = function_docstring_expr(func);
    if let Some(lowered) = try_lower_function_to_blockpy_bundle(
        context,
        module_scope,
        function_identity_by_node,
        func,
        current_parent,
        reserved_temp_names_stack,
        used_label_prefixes,
        next_block_id,
    ) {
        let rewrite_plan = plan_and_rewrite_lowered_function_stmt(
            parent_hoisted,
            func,
            &lowered.main_function,
            function_identity_by_node,
            current_parent,
            needs_cell_sync,
            entering_module_init,
            has_parent_hoisted_scope,
            function_hoisted,
            doc_expr,
        )
        .expect("failed to build BB function binding");
        let _ = context;
        push_lowered_blockpy_function_bundle(
            lowered_blockpy_module,
            lowered,
            rewrite_plan.binding_target,
        );
        return Some(rewrite_plan.rewrite.replacement);
    }

    plan_and_rewrite_non_lowered_function_stmt(
        context,
        func,
        function_identity_by_node,
        current_parent,
        needs_cell_sync,
        function_hoisted,
        doc_expr.unwrap_or_else(|| py_expr!("None")),
        || next_temp_from_counter(reserved_temp_names_stack, "fn_local", next_block_id),
    )
}

fn next_temp_from_counter(
    reserved_temp_names_stack: &mut Vec<HashSet<String>>,
    prefix: &str,
    next_id: &mut usize,
) -> String {
    loop {
        let current = *next_id;
        *next_id += 1;
        let candidate = format!("_dp_{prefix}_{current}");
        let collides = reserved_temp_names_stack
            .last()
            .is_some_and(|names| names.contains(candidate.as_str()));
        if collides {
            continue;
        }
        if let Some(names) = reserved_temp_names_stack.last_mut() {
            names.insert(candidate.clone());
        }
        return candidate;
    }
}

pub(crate) fn make_dp_tuple(items: Vec<Expr>) -> Expr {
    let Expr::Call(mut call) = py_expr!("__dp_tuple()") else {
        panic!("expected call expression for __dp_tuple");
    };
    call.arguments.args = items.into();
    Expr::Call(call)
}

pub(crate) fn name_expr(name: &str) -> Option<Expr> {
    parse_expression(name)
        .ok()
        .map(|expr| *expr.into_syntax().body)
}
