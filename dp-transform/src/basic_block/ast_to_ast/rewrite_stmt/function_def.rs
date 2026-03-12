use crate::basic_block::annotation_export::{
    build_lowered_annotation_helper_binding, is_annotation_helper_name,
    prepare_non_lowered_annotationlib_function,
};
use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::ast_to_ast::rewrite_stmt;
use crate::basic_block::ast_to_ast::scope::{
    analyze_module_scope, cell_name, is_internal_symbol, Scope,
};
use crate::basic_block::bb_ir::{BbFunctionKind, BindingTarget, FunctionId};
use crate::basic_block::block_py::dataflow::analyze_blockpy_use_def;
use crate::basic_block::block_py::state::collect_cell_slots;
use crate::basic_block::block_py::state::collect_parameter_names;
use crate::basic_block::blockpy_to_bb::{
    push_lowered_blockpy_callable_def_bundle, LoweredBlockPyModuleBundle,
};
use crate::basic_block::expr_utils::{make_dp_tuple, name_expr};
use crate::basic_block::function_identity::{
    is_module_init_temp_name, resolve_runtime_function_identity, FunctionIdentity,
    FunctionIdentityByNode,
};
use crate::basic_block::function_lowering::{
    function_docstring_expr, try_lower_function_to_blockpy_bundle,
};
use crate::basic_block::ruff_to_blockpy::LoweredBlockPyFunction;
use crate::template::into_body;
use crate::transformer::{walk_stmt, Transformer};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, name::Name, Expr, NodeIndex, Stmt, StmtBody};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

struct FunctionScopeFrame {
    name: String,
    parent_name: Option<String>,
    entering_module_init: bool,
    has_parent_hoisted_scope: bool,
    needs_cell_sync: bool,
    cell_bindings: HashSet<String>,
    hoisted_to_parent: Vec<Stmt>,
}

struct BlockPyModuleRewriter<'a> {
    context: &'a Context,
    module_scope: Arc<Scope>,
    function_identity_by_node: HashMap<NodeIndex, FunctionIdentity>,
    next_block_id: usize,
    next_function_id: usize,
    reserved_temp_names_stack: Vec<HashSet<String>>,
    used_label_prefixes: HashMap<String, usize>,
    function_scope_stack: Vec<FunctionScopeFrame>,
    lowered_function_binding_by_id: HashMap<FunctionId, LoweredFunctionBindingPlan>,
    lowered_blockpy_module: LoweredBlockPyModuleBundle,
}

enum LoweredFunctionPlacementPlan {
    ReplaceWith(Stmt),
    HoistToParent {
        replacement: Stmt,
        hoisted_to_parent: Vec<Stmt>,
    },
}

enum NonLoweredFunctionPlacementPlan {
    ReplaceWith(Stmt),
    PrependBody(Vec<Stmt>),
    LeaveInPlace,
}

#[derive(Clone, Copy)]
struct LoweredFunctionBindingPlan {
    target: BindingTarget,
    needs_cell_sync: bool,
}

struct LoweredFunctionInstantiationPlan {
    identity: FunctionIdentity,
    binding: LoweredFunctionBindingPlan,
}

struct LoweredFunctionRewriteResult {
    replacement: Stmt,
}

struct LoweredFunctionVisitPlan {
    binding: LoweredFunctionBindingPlan,
    rewrite: LoweredFunctionRewriteResult,
}

enum NonLoweredFunctionBindingPlan {
    LeaveLocal,
    CellSyncOnly,
    Rebind { target: BindingTarget },
}

enum NonLoweredLocalNamePlan {
    KeepOriginal,
    UseFreshTemp,
}

struct NonLoweredFunctionInstantiationPlan {
    identity: FunctionIdentity,
    binding: NonLoweredFunctionBindingPlan,
    local_name_plan: Option<NonLoweredLocalNamePlan>,
}

// Function-definition rewriting stays in one tree pass, but the instantiation
// machinery is grouped here so the later binding split has one obvious home.
fn build_lowered_function_instantiation_expr(
    lowered: &LoweredBlockPyFunction,
    doc_expr: Option<Expr>,
    annotate_fn_expr: Option<Expr>,
) -> Option<Expr> {
    let entry_label = lowered.callable_def.entry_label();
    let entry_ref_expr = py_expr!("{entry:literal}", entry = entry_label);
    let function_id = lowered.callable_def.function_id.0;
    let param_names: HashSet<String> = collect_parameter_names(&lowered.callable_def.params)
        .into_iter()
        .collect();
    let generator_lifted_state_names: HashSet<&str> = lowered
        .closure_layout
        .as_ref()
        .map(|layout| {
            layout
                .cellvars
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
                .freevars
                .iter()
                .chain(layout.cellvars.iter())
                .chain(layout.runtime_cells.iter())
                .map(|slot| slot.storage_name.as_str())
                .collect()
        })
        .unwrap_or_default();
    let locally_assigned: HashSet<String> = lowered
        .callable_def
        .blocks
        .iter()
        .flat_map(|block| analyze_blockpy_use_def(block).1.into_iter())
        .collect();
    let mut closure_items = Vec::new();
    for entry_name in &lowered.callable_def.entry_liveins {
        if param_names.contains(entry_name) {
            closure_items.push(py_expr!("{value:literal}", value = entry_name.as_str(),));
        } else if entry_name == "_dp_classcell"
            || (entry_name.starts_with("_dp_cell_")
                && !lowered.callable_def.local_cell_slots.contains(entry_name))
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
        "__dp_make_function({entry:expr}, {function_id:literal}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, {module_globals:expr}, {module_name:expr}, {doc:expr}, {annotate_fn:expr})",
        entry = entry_ref_expr.clone(),
        function_id = function_id,
        name = lowered.callable_def.display_name.as_str(),
        qualname = lowered.callable_def.qualname.as_str(),
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
                "__dp_def_async_gen({resume:expr}, {function_id:literal}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, __dp_globals(), __name__, {doc:expr}, {annotate_fn:expr})",
                resume = entry_ref_expr.clone(),
                function_id = function_id,
                name = lowered.callable_def.display_name.as_str(),
                qualname = lowered.callable_def.qualname.as_str(),
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
                    "__dp_def_coro_from_gen({resume:expr}, {function_id:literal}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, __dp_globals(), __name__, {doc:expr}, {annotate_fn:expr})",
                    resume = entry_ref_expr,
                    function_id = function_id,
                    name = lowered.callable_def.display_name.as_str(),
                    qualname = lowered.callable_def.qualname.as_str(),
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

pub(crate) fn rewrite_ast_to_lowered_blockpy_module(
    context: &Context,
    module: &mut StmtBody,
    function_identity_by_node: FunctionIdentityByNode,
) -> LoweredBlockPyModuleBundle {
    let module_scope = analyze_module_scope(module);
    let function_identity_by_node = function_identity_by_node
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
        .collect();
    let mut rewriter = BlockPyModuleRewriter {
        context,
        module_scope,
        function_identity_by_node,
        next_block_id: 0,
        next_function_id: 0,
        reserved_temp_names_stack: Vec::new(),
        used_label_prefixes: HashMap::new(),
        function_scope_stack: Vec::new(),
        lowered_function_binding_by_id: HashMap::new(),
        lowered_blockpy_module: LoweredBlockPyModuleBundle {
            callable_defs: Vec::new(),
            module_init: Some("_dp_module_init".to_string()),
        },
    };
    rewriter.visit_body(module);
    crate::basic_block::ast_to_ast::simplify::strip_generated_passes(context, module);
    rewriter.lowered_blockpy_module
}

fn build_binding_stmt(target: BindingTarget, bind_name: &str, value: Expr) -> Stmt {
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

fn build_generated_instantiation_assign_stmt(bind_name: &str, value: Expr) -> Stmt {
    py_stmt!("{name:id} = {value:expr}", name = bind_name, value = value)
}

fn build_cell_sync_stmt(bind_name: &str) -> Stmt {
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

fn plan_lowered_function_binding(
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

fn plan_lowered_function_instantiation(
    func: &ast::StmtFunctionDef,
    function_identity_by_node: &HashMap<NodeIndex, FunctionIdentity>,
    current_parent: Option<&str>,
    needs_cell_sync: bool,
) -> LoweredFunctionInstantiationPlan {
    let identity =
        resolve_runtime_function_identity(func, function_identity_by_node, current_parent);
    let binding = plan_lowered_function_binding(
        identity.binding_target,
        identity.bind_name.as_str(),
        identity.qualname.as_str(),
        needs_cell_sync,
    );
    LoweredFunctionInstantiationPlan { identity, binding }
}

fn plan_non_lowered_function_binding(
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

fn plan_non_lowered_local_name(
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

fn plan_non_lowered_function_instantiation(
    func: &ast::StmtFunctionDef,
    function_identity_by_node: &HashMap<NodeIndex, FunctionIdentity>,
    current_parent: Option<&str>,
    needs_cell_sync: bool,
    is_annotation_helper: bool,
) -> NonLoweredFunctionInstantiationPlan {
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
    NonLoweredFunctionInstantiationPlan {
        identity,
        binding,
        local_name_plan,
    }
}

fn build_updated_function_binding_stmt(
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

fn build_non_lowered_binding_stmt(
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

fn plan_lowered_function_placement(
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

fn plan_non_lowered_function_placement(
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

fn apply_lowered_function_placement(
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

fn apply_non_lowered_function_placement(
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

fn build_lowered_function_instantiation_stmt(
    func: &ast::StmtFunctionDef,
    lowered: &LoweredBlockPyFunction,
    bind_name: &str,
    doc_expr: Option<Expr>,
) -> Option<Stmt> {
    let annotate_helper = build_lowered_annotation_helper_binding(func, bind_name);
    let annotate_fn_expr = annotate_helper
        .as_ref()
        .map(|(_, annotate_fn_expr)| annotate_fn_expr.clone());
    let base_expr = build_lowered_function_instantiation_expr(lowered, doc_expr, annotate_fn_expr)?;
    let decorated = rewrite_stmt::decorator::rewrite(func.decorator_list.clone(), base_expr);
    let assign_stmt = build_generated_instantiation_assign_stmt(bind_name, decorated);
    let mut stmts = Vec::new();
    if let Some((helper_stmt, _)) = annotate_helper {
        stmts.push(helper_stmt);
    }
    stmts.push(assign_stmt);
    if stmts.len() == 1 {
        stmts.into_iter().next()
    } else {
        Some(into_body(stmts))
    }
}

fn rewrite_lowered_function_instantiation_stmt(
    parent_hoisted: Option<&mut Vec<Stmt>>,
    func: &ast::StmtFunctionDef,
    lowered: &LoweredBlockPyFunction,
    instantiation_plan: &LoweredFunctionInstantiationPlan,
    entering_module_init: bool,
    has_parent_hoisted_scope: bool,
    function_hoisted: Vec<Stmt>,
    doc_expr: Option<Expr>,
) -> Option<LoweredFunctionRewriteResult> {
    let binding_stmt = build_lowered_function_instantiation_stmt(
        func,
        lowered,
        instantiation_plan.identity.bind_name.as_str(),
        doc_expr,
    )?;
    let replacement = apply_lowered_function_placement(
        parent_hoisted,
        plan_lowered_function_placement(
            instantiation_plan.identity.bind_name.as_str(),
            entering_module_init,
            has_parent_hoisted_scope,
            function_hoisted,
            binding_stmt,
        ),
    );
    Some(LoweredFunctionRewriteResult { replacement })
}

fn plan_and_rewrite_lowered_function_instantiation(
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
    let instantiation_plan = plan_lowered_function_instantiation(
        func,
        function_identity_by_node,
        current_parent,
        needs_cell_sync,
    );
    let rewrite = rewrite_lowered_function_instantiation_stmt(
        parent_hoisted,
        func,
        lowered,
        &instantiation_plan,
        entering_module_init,
        has_parent_hoisted_scope,
        function_hoisted,
        doc_expr,
    )?;
    Some(LoweredFunctionVisitPlan {
        binding: instantiation_plan.binding,
        rewrite,
    })
}

fn rewrite_non_lowered_function_instantiation(
    func: &mut ast::StmtFunctionDef,
    instantiation_plan: NonLoweredFunctionInstantiationPlan,
    function_hoisted: Vec<Stmt>,
    doc: Expr,
    mut next_temp: impl FnMut() -> String,
) -> Option<Stmt> {
    let fresh_local_name = match instantiation_plan.local_name_plan {
        Some(NonLoweredLocalNamePlan::UseFreshTemp) => Some(next_temp()),
        Some(NonLoweredLocalNamePlan::KeepOriginal) | None => None,
    };
    let binding_stmt = build_non_lowered_binding_stmt(
        func,
        instantiation_plan.identity.bind_name.as_str(),
        instantiation_plan.identity.qualname.as_str(),
        instantiation_plan.identity.display_name.as_str(),
        instantiation_plan.binding,
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

fn plan_and_rewrite_non_lowered_function_instantiation(
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
    let instantiation_plan = plan_non_lowered_function_instantiation(
        func,
        function_identity_by_node,
        current_parent,
        needs_cell_sync,
        is_annotation_helper_name(func.name.id.as_str()),
    );
    rewrite_non_lowered_function_instantiation(
        func,
        instantiation_plan,
        function_hoisted,
        doc,
        next_temp,
    )
}

#[allow(clippy::too_many_arguments)]
fn rewrite_function_def_stmt_via_blockpy(
    context: &Context,
    module_scope: &Arc<Scope>,
    lowered_blockpy_module: &mut LoweredBlockPyModuleBundle,
    lowered_function_binding_by_id: &mut HashMap<FunctionId, LoweredFunctionBindingPlan>,
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
    next_function_id: &mut usize,
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
        next_function_id,
    ) {
        let rewrite_plan = plan_and_rewrite_lowered_function_instantiation(
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
        lowered_function_binding_by_id.insert(
            lowered.main_function.callable_def.function_id,
            rewrite_plan.binding,
        );
        push_lowered_blockpy_callable_def_bundle(
            lowered_blockpy_module,
            lowered,
            rewrite_plan.binding.target,
        );
        return Some(rewrite_plan.rewrite.replacement);
    }

    plan_and_rewrite_non_lowered_function_instantiation(
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

fn function_id_literal(expr: &Expr) -> Option<FunctionId> {
    let Expr::NumberLiteral(number) = expr else {
        return None;
    };
    let ast::Number::Int(value) = &number.value else {
        return None;
    };
    value.as_usize().map(FunctionId)
}

fn generated_function_id_from_expr(expr: &Expr) -> Option<FunctionId> {
    match expr {
        Expr::Call(call) => {
            if let Expr::Name(name) = call.func.as_ref() {
                let helper_name = name.id.as_str();
                if helper_name == "__dp_make_function"
                    || helper_name == "__dp_def_async_gen"
                    || helper_name == "__dp_def_coro_from_gen"
                {
                    return call.arguments.args.get(1).and_then(function_id_literal);
                }
                if helper_name == "__dp_mark_coroutine_function" {
                    return call
                        .arguments
                        .args
                        .first()
                        .and_then(generated_function_id_from_expr);
                }
            }
            call.arguments
                .args
                .iter()
                .find_map(generated_function_id_from_expr)
                .or_else(|| {
                    call.arguments
                        .keywords
                        .iter()
                        .find_map(|keyword| generated_function_id_from_expr(&keyword.value))
                })
        }
        _ => None,
    }
}

fn rewrite_generated_lowered_function_binding_assign(
    assign: &ast::StmtAssign,
    plan: &LoweredFunctionBindingPlan,
) -> Option<Stmt> {
    let [Expr::Name(target_name)] = assign.targets.as_slice() else {
        panic!("generated function instantiation assignment should target one name");
    };
    let bind_name = target_name.id.as_str();
    let value = assign.value.as_ref().clone();
    match plan.target {
        BindingTarget::Local => {
            if plan.needs_cell_sync {
                Some(into_body(vec![
                    Stmt::Assign(assign.clone()),
                    build_cell_sync_stmt(bind_name),
                ]))
            } else {
                None
            }
        }
        BindingTarget::ModuleGlobal | BindingTarget::ClassNamespace => {
            Some(build_binding_stmt(plan.target, bind_name, value))
        }
    }
}

struct LoweredFunctionBindingRewriter<'a> {
    binding_by_id: &'a HashMap<FunctionId, LoweredFunctionBindingPlan>,
}

impl Transformer for LoweredFunctionBindingRewriter<'_> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if let Stmt::Assign(assign) = stmt {
            if let Some(function_id) = generated_function_id_from_expr(assign.value.as_ref()) {
                if let Some(plan) = self.binding_by_id.get(&function_id) {
                    if let Some(rewritten) =
                        rewrite_generated_lowered_function_binding_assign(assign, plan)
                    {
                        *stmt = rewritten;
                        return;
                    }
                }
            }
        }
        walk_stmt(self, stmt);
    }
}

fn rewrite_generated_lowered_function_bindings_in_stmt_slice(
    stmts: &mut [Stmt],
    binding_by_id: &HashMap<FunctionId, LoweredFunctionBindingPlan>,
) {
    if binding_by_id.is_empty() {
        return;
    }
    let mut rewriter = LoweredFunctionBindingRewriter { binding_by_id };
    for stmt in stmts {
        rewriter.visit_stmt(stmt);
    }
}

fn rewrite_generated_lowered_function_bindings_in_boxed_stmt_slice(
    stmts: &mut [Box<Stmt>],
    binding_by_id: &HashMap<FunctionId, LoweredFunctionBindingPlan>,
) {
    if binding_by_id.is_empty() {
        return;
    }
    let mut rewriter = LoweredFunctionBindingRewriter { binding_by_id };
    for stmt in stmts {
        rewriter.visit_stmt(stmt.as_mut());
    }
}

impl BlockPyModuleRewriter<'_> {
    fn walk_function_def_with_scope(&mut self, stmt: &mut Stmt) -> Option<FunctionScopeFrame> {
        let Stmt::FunctionDef(func) = stmt else {
            return None;
        };
        let fn_name = func.name.id.to_string();
        let bind_name = func.name.id.to_string();
        let parent_name = self
            .function_scope_stack
            .last()
            .map(|frame| frame.name.clone());
        let entering_module_init = is_module_init_temp_name(fn_name.as_str());
        let has_parent_hoisted_scope = !self.function_scope_stack.is_empty();
        let cell_bindings = collect_cell_slots(&func.body.body)
            .into_iter()
            .filter_map(|slot| slot.strip_prefix("_dp_cell_").map(str::to_string))
            .collect::<HashSet<_>>();
        let needs_cell_sync = self
            .function_scope_stack
            .last()
            .map(|frame| frame.cell_bindings.contains(bind_name.as_str()))
            .unwrap_or(false);
        self.function_scope_stack.push(FunctionScopeFrame {
            name: fn_name,
            parent_name,
            entering_module_init,
            has_parent_hoisted_scope,
            needs_cell_sync,
            cell_bindings,
            hoisted_to_parent: Vec::new(),
        });
        walk_stmt(self, stmt);
        self.function_scope_stack.pop()
    }

    fn visit_function_def_stmt(&mut self, stmt: &mut Stmt) {
        let Some(state) = self.walk_function_def_with_scope(stmt) else {
            return;
        };
        if let Stmt::FunctionDef(func) = stmt {
            if let Some(replacement) = self.rewrite_visited_function_def(func, state) {
                *stmt = replacement;
            }
        }
    }

    fn rewrite_visited_function_def(
        &mut self,
        func: &mut ast::StmtFunctionDef,
        mut state: FunctionScopeFrame,
    ) -> Option<Stmt> {
        // Nested lowered defs are rewritten into plain instantiation assignments as we walk
        // the tree. Rewrite those assignments to their real binding form before lowering this
        // enclosing function so its BlockPy/BB sees the final binding behavior.
        rewrite_generated_lowered_function_bindings_in_boxed_stmt_slice(
            &mut func.body.body,
            &self.lowered_function_binding_by_id,
        );
        rewrite_generated_lowered_function_bindings_in_stmt_slice(
            &mut state.hoisted_to_parent,
            &self.lowered_function_binding_by_id,
        );
        rewrite_function_def_stmt_via_blockpy(
            self.context,
            &self.module_scope,
            &mut self.lowered_blockpy_module,
            &mut self.lowered_function_binding_by_id,
            self.function_scope_stack
                .last_mut()
                .map(|frame| &mut frame.hoisted_to_parent),
            &self.function_identity_by_node,
            func,
            state.parent_name.as_deref(),
            state.needs_cell_sync,
            state.entering_module_init,
            state.has_parent_hoisted_scope,
            state.hoisted_to_parent,
            &mut self.reserved_temp_names_stack,
            &mut self.used_label_prefixes,
            &mut self.next_block_id,
            &mut self.next_function_id,
        )
    }
}

impl Transformer for BlockPyModuleRewriter<'_> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if matches!(stmt, Stmt::FunctionDef(_)) {
            self.visit_function_def_stmt(stmt);
            return;
        }

        walk_stmt(self, stmt);
    }
}
