use crate::basic_block::annotation_export::{
    build_lowered_annotation_helper_binding, is_annotation_helper_name,
    prepare_non_lowered_annotationlib_function,
};
use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::ast_to_ast::rewrite_stmt;
use crate::basic_block::ast_to_ast::scope::{
    analyze_module_scope, cell_name, is_internal_symbol, Scope,
};
use crate::basic_block::bb_ir::{BbFunctionKind, BindingTarget};
use crate::basic_block::block_py::dataflow::analyze_blockpy_use_def;
use crate::basic_block::block_py::state::collect_cell_slots;
use crate::basic_block::block_py::state::collect_parameter_names;
use crate::basic_block::blockpy_to_bb::{LoweredBlockPyModuleBundle, LoweredCallableDef};
use crate::basic_block::expr_utils::{make_dp_tuple, name_expr};
use crate::basic_block::function_identity::{
    collect_function_identity_private, is_module_init_temp_name, resolve_runtime_function_identity,
    FunctionIdentity,
};
use crate::basic_block::function_lowering::{
    function_docstring_expr, try_lower_function_to_blockpy_bundle,
};
use crate::basic_block::param_specs::{
    collect_function_param_specs, function_param_specs_to_expr, FunctionParamSpec,
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

enum LoweredFunctionInstantiationKind {
    DirectFunction,
    MarkCoroutineFunction,
    AsyncGeneratorDefinition,
    CoroutineFromGeneratorDefinition,
}

#[derive(Clone)]
enum LoweredFunctionCaptureItem {
    Symbol(String),
    BoundValue { name: String, value_expr: Expr },
}

struct LoweredFunctionInstantiationData {
    entry_label: String,
    function_id: usize,
    name: String,
    qualname: String,
    captures: Vec<LoweredFunctionCaptureItem>,
    decorator_exprs: Vec<Expr>,
    param_specs: Vec<FunctionParamSpec>,
    doc_expr: Expr,
    annotate_fn_expr: Expr,
    kind: LoweredFunctionInstantiationKind,
}

struct LoweredFunctionRewriteResult {
    replacement: Stmt,
}

struct LoweredFunctionVisitPlan {
    main_binding_target: BindingTarget,
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
fn capture_items_to_expr(captures: &[LoweredFunctionCaptureItem]) -> Expr {
    make_dp_tuple(
        captures
            .iter()
            .map(|capture| match capture {
                LoweredFunctionCaptureItem::Symbol(name) => {
                    py_expr!("{value:literal}", value = name.as_str())
                }
                LoweredFunctionCaptureItem::BoundValue { name, value_expr } => make_dp_tuple(vec![
                    py_expr!("{value:literal}", value = name.as_str()),
                    value_expr.clone(),
                ]),
            })
            .collect(),
    )
}

fn push_lowered_blockpy_callable_def_bundle(
    out: &mut LoweredBlockPyModuleBundle,
    bundle: crate::basic_block::ruff_to_blockpy::LoweredBlockPyFunctionBundle,
    main_binding_target: BindingTarget,
) {
    out.callable_defs
        .extend(
            bundle
                .helper_functions
                .into_iter()
                .map(|helper| LoweredCallableDef {
                    callable_def: helper,
                    binding_target: BindingTarget::Local,
                }),
        );
    out.callable_defs.push(LoweredCallableDef {
        callable_def: bundle.main_function,
        binding_target: main_binding_target,
    });
}

fn build_lowered_function_instantiation_data(
    lowered: &LoweredBlockPyFunction,
    decorator_exprs: Vec<Expr>,
    annotate_fn_expr: Option<Expr>,
) -> Option<LoweredFunctionInstantiationData> {
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
    let mut captures = Vec::new();
    for entry_name in &lowered.callable_def.entry_liveins {
        if param_names.contains(entry_name) {
            captures.push(LoweredFunctionCaptureItem::Symbol(entry_name.clone()));
        } else if entry_name == "_dp_classcell"
            || (entry_name.starts_with("_dp_cell_")
                && !lowered.callable_def.local_cell_slots.contains(entry_name))
        {
            captures.push(LoweredFunctionCaptureItem::BoundValue {
                name: entry_name.clone(),
                value_expr: name_expr(entry_name.as_str())?,
            });
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
            captures.push(LoweredFunctionCaptureItem::BoundValue {
                name: entry_name.clone(),
                value_expr: name_expr(entry_name.as_str())?,
            });
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
            captures.push(LoweredFunctionCaptureItem::Symbol(entry_name.clone()));
        } else if !entry_name.starts_with("_dp_") && !locally_assigned.contains(entry_name) {
            captures.push(LoweredFunctionCaptureItem::BoundValue {
                name: entry_name.clone(),
                value_expr: name_expr(entry_name.as_str())?,
            });
        } else {
            captures.push(LoweredFunctionCaptureItem::Symbol(entry_name.clone()));
        }
    }
    let doc_expr = lowered
        .callable_def
        .doc
        .clone()
        .map(Into::into)
        .unwrap_or_else(|| py_expr!("None"));
    let annotate_fn_expr = annotate_fn_expr.unwrap_or_else(|| py_expr!("None"));
    let kind = match &lowered.bb_kind {
        BbFunctionKind::Function => {
            if lowered.is_coroutine {
                LoweredFunctionInstantiationKind::MarkCoroutineFunction
            } else {
                LoweredFunctionInstantiationKind::DirectFunction
            }
        }
        BbFunctionKind::AsyncGenerator { closure_state, .. } => {
            if *closure_state {
                LoweredFunctionInstantiationKind::DirectFunction
            } else {
                LoweredFunctionInstantiationKind::AsyncGeneratorDefinition
            }
        }
        BbFunctionKind::Generator { closure_state, .. } => {
            if *closure_state {
                if lowered.is_coroutine {
                    LoweredFunctionInstantiationKind::MarkCoroutineFunction
                } else {
                    LoweredFunctionInstantiationKind::DirectFunction
                }
            } else if lowered.is_coroutine {
                LoweredFunctionInstantiationKind::CoroutineFromGeneratorDefinition
            } else {
                panic!(
                    "non-closure-backed sync generator lowering is unreachable; \
                     generated comprehension helpers are async-only"
                )
            }
        }
    };
    Some(LoweredFunctionInstantiationData {
        entry_label: lowered.callable_def.entry_label().to_string(),
        function_id: lowered.callable_def.function_id.0,
        name: lowered.callable_def.display_name.clone(),
        qualname: lowered.callable_def.qualname.clone(),
        captures,
        decorator_exprs,
        param_specs: collect_function_param_specs(&lowered.callable_def.params),
        doc_expr,
        annotate_fn_expr,
        kind,
    })
}

fn build_lowered_function_instantiation_expr(data: &LoweredFunctionInstantiationData) -> Expr {
    let entry_ref_expr = py_expr!("{entry:literal}", entry = data.entry_label.as_str());
    let capture_expr = capture_items_to_expr(&data.captures);
    let param_specs_expr = function_param_specs_to_expr(&data.param_specs);
    let function_entry_expr = py_expr!(
        "__dp_make_function({entry:expr}, {function_id:literal}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, {module_globals:expr}, {module_name:expr}, {doc:expr}, {annotate_fn:expr})",
        entry = entry_ref_expr.clone(),
        function_id = data.function_id,
        name = data.name.as_str(),
        qualname = data.qualname.as_str(),
        closure = capture_expr.clone(),
        params = param_specs_expr.clone(),
        module_globals = py_expr!("__dp_globals()"),
        module_name = py_expr!("__name__"),
        doc = data.doc_expr.clone(),
        annotate_fn = data.annotate_fn_expr.clone(),
    );
    let base_function_expr = match data.kind {
        LoweredFunctionInstantiationKind::DirectFunction => function_entry_expr,
        LoweredFunctionInstantiationKind::MarkCoroutineFunction => py_expr!(
            "__dp_mark_coroutine_function({func:expr})",
            func = function_entry_expr,
        ),
        LoweredFunctionInstantiationKind::AsyncGeneratorDefinition => py_expr!(
            "__dp_def_async_gen({resume:expr}, {function_id:literal}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, __dp_globals(), __name__, {doc:expr}, {annotate_fn:expr})",
            resume = entry_ref_expr,
            function_id = data.function_id,
            name = data.name.as_str(),
            qualname = data.qualname.as_str(),
            closure = capture_expr.clone(),
            params = param_specs_expr.clone(),
            doc = data.doc_expr.clone(),
            annotate_fn = data.annotate_fn_expr.clone(),
        ),
        LoweredFunctionInstantiationKind::CoroutineFromGeneratorDefinition => py_expr!(
            "__dp_def_coro_from_gen({resume:expr}, {function_id:literal}, {name:literal}, {qualname:literal}, {closure:expr}, {params:expr}, __dp_globals(), __name__, {doc:expr}, {annotate_fn:expr})",
            resume = entry_ref_expr,
            function_id = data.function_id,
            name = data.name.as_str(),
            qualname = data.qualname.as_str(),
            closure = capture_expr.clone(),
            params = param_specs_expr.clone(),
            doc = data.doc_expr.clone(),
            annotate_fn = data.annotate_fn_expr.clone(),
        ),
    };
    rewrite_stmt::decorator::rewrite_exprs(data.decorator_exprs.clone(), base_function_expr)
}

#[cfg(test)]
mod tests {
    use super::{capture_items_to_expr, LoweredFunctionCaptureItem};

    #[test]
    fn capture_items_render_as_symbol_or_name_value_pairs() {
        let expr = capture_items_to_expr(&[
            LoweredFunctionCaptureItem::Symbol("x".to_string()),
            LoweredFunctionCaptureItem::BoundValue {
                name: "y".to_string(),
                value_expr: crate::py_expr!("z"),
            },
        ]);
        assert_eq!(
            crate::ruff_ast_to_string(&expr).trim(),
            "__dp_tuple(\"x\", __dp_tuple(\"y\", z))"
        );
    }
}

pub(crate) fn rewrite_ast_to_lowered_blockpy_module(
    context: &Context,
    module: &mut StmtBody,
) -> LoweredBlockPyModuleBundle {
    crate::basic_block::ast_to_ast::simplify::flatten(module);
    let module_scope = analyze_module_scope(module);
    let function_identity_by_node = collect_function_identity_private(module, module_scope.clone());
    let mut rewriter = BlockPyModuleRewriter {
        context,
        module_scope,
        function_identity_by_node,
        next_block_id: 0,
        next_function_id: 0,
        reserved_temp_names_stack: Vec::new(),
        used_label_prefixes: HashMap::new(),
        function_scope_stack: Vec::new(),
        lowered_blockpy_module: LoweredBlockPyModuleBundle {
            callable_defs: Vec::new(),
            module_init: Some("_dp_module_init".to_string()),
        },
    };
    rewriter.visit_body(module);
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
    decorator_exprs: Vec<Expr>,
) -> Stmt {
    let updated = py_expr!(
        "__dp_update_fn({name:id}, {qualname:literal}, {display_name:literal}, {doc:expr})",
        name = local_name,
        qualname = qualname,
        display_name = display_name,
        doc = doc,
    );
    let value = rewrite_stmt::decorator::rewrite_exprs(decorator_exprs, updated);
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
            let decorator_exprs =
                rewrite_stmt::decorator::into_exprs(std::mem::take(&mut func.decorator_list));
            Some(build_updated_function_binding_stmt(
                target,
                bind_name,
                local_name.as_str(),
                qualname,
                display_name,
                doc,
                decorator_exprs,
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

fn build_lowered_function_binding_stmt(
    bind_name: &str,
    value: Expr,
    binding_plan: LoweredFunctionBindingPlan,
) -> Stmt {
    match binding_plan.target {
        BindingTarget::Local => {
            let assign_stmt = py_stmt!("{name:id} = {value:expr}", name = bind_name, value = value);
            if binding_plan.needs_cell_sync {
                into_body(vec![assign_stmt, build_cell_sync_stmt(bind_name)])
            } else {
                assign_stmt
            }
        }
        BindingTarget::ModuleGlobal | BindingTarget::ClassNamespace => {
            build_binding_stmt(binding_plan.target, bind_name, value)
        }
    }
}

fn build_lowered_function_instantiation_stmt(
    func: &ast::StmtFunctionDef,
    lowered: &LoweredBlockPyFunction,
    instantiation_plan: &LoweredFunctionInstantiationPlan,
) -> Option<Stmt> {
    let bind_name = instantiation_plan.identity.bind_name.as_str();
    let annotate_helper = build_lowered_annotation_helper_binding(func, bind_name);
    let annotate_fn_expr = annotate_helper
        .as_ref()
        .map(|(_, annotate_fn_expr)| annotate_fn_expr.clone());
    let instantiation_data = build_lowered_function_instantiation_data(
        lowered,
        rewrite_stmt::decorator::collect_exprs(&func.decorator_list),
        annotate_fn_expr,
    )?;
    let decorated = build_lowered_function_instantiation_expr(&instantiation_data);
    let binding_stmt =
        build_lowered_function_binding_stmt(bind_name, decorated, instantiation_plan.binding);
    let mut stmts = Vec::new();
    if let Some((helper_stmt, _)) = annotate_helper {
        stmts.push(helper_stmt);
    }
    stmts.push(binding_stmt);
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
) -> Option<LoweredFunctionRewriteResult> {
    let binding_stmt =
        build_lowered_function_instantiation_stmt(func, lowered, instantiation_plan)?;
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
    )?;
    Some(LoweredFunctionVisitPlan {
        main_binding_target: instantiation_plan.binding.target,
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
        )
        .expect("failed to build BB function binding");
        push_lowered_blockpy_callable_def_bundle(
            lowered_blockpy_module,
            lowered,
            rewrite_plan.main_binding_target,
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
        state: FunctionScopeFrame,
    ) -> Option<Stmt> {
        let parent_hoisted = self
            .function_scope_stack
            .last_mut()
            .map(|parent_frame| &mut parent_frame.hoisted_to_parent);
        rewrite_function_def_stmt_via_blockpy(
            self.context,
            &self.module_scope,
            &mut self.lowered_blockpy_module,
            parent_hoisted,
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
