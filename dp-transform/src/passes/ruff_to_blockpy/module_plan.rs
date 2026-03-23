use crate::block_py::dataflow::analyze_blockpy_use_def;
use crate::block_py::param_specs::{collect_param_spec_and_defaults, param_defaults_to_expr};
use crate::block_py::state::collect_cell_slots;
use crate::block_py::BindingTarget;
use crate::block_py::{
    BlockPyCallableFacts, BlockPyFunction, BlockPyFunctionKind, BlockPyModule, FunctionName,
};
use crate::passes::annotation_export::{
    build_lowered_annotation_helper_binding, is_annotation_helper_name,
    prepare_non_lowered_annotationlib_function, rewrite_annotation_helper_defs_as_exec_calls,
    should_keep_non_lowered_for_annotationlib,
};
use crate::passes::ast_symbol_analysis::{
    collect_bound_names, collect_explicit_global_or_nonlocal_names,
};
use crate::passes::ast_to_ast::body::{suite_mut, suite_ref, take_suite, Suite};
use crate::passes::ast_to_ast::context::Context;
use crate::passes::ast_to_ast::expr_utils::{make_dp_tuple, name_expr};
use crate::passes::ast_to_ast::rewrite_stmt;
use crate::passes::ast_to_ast::scope::{analyze_module_scope, cell_name, is_internal_symbol};
use crate::passes::ast_to_ast::semantic::SemanticAstState;
use crate::passes::RuffBlockPyPass;

use crate::passes::function_identity::{
    collect_function_identity_private, is_module_init_temp_name, resolve_runtime_function_identity,
    FunctionIdentity,
};
use crate::transformer::{walk_expr, walk_stmt, Transformer};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, name::Name, Expr, NodeIndex, Stmt};
use std::collections::{HashMap, HashSet};

use super::{
    build_blockpy_callable_def_from_runtime_input, rewrite_deleted_name_loads,
    take_next_function_id,
};

struct FunctionScopeFrame {
    name: String,
    parent_name: Option<String>,
    cell_bindings: HashSet<String>,
    entering_module_init: bool,
    has_parent_hoisted_scope: bool,
    needs_cell_sync: bool,
    hoisted_to_parent: Vec<Stmt>,
}

struct BlockPyModuleRewriter<'a> {
    context: &'a Context,
    semantic_state: &'a SemanticAstState,
    function_identity_by_node: HashMap<NodeIndex, FunctionIdentity>,
    next_block_id: usize,
    next_function_id: usize,
    reserved_temp_names_stack: Vec<HashSet<String>>,
    function_scope_stack: Vec<FunctionScopeFrame>,
    callable_defs: Vec<BlockPyFunction<RuffBlockPyPass>>,
}

enum LoweredFunctionPlacementPlan {
    ReplaceWith(Vec<Stmt>),
    HoistToParent {
        replacement: Vec<Stmt>,
        hoisted_to_parent: Vec<Stmt>,
    },
}

enum NonLoweredFunctionPlacementPlan {
    ReplaceWith(Vec<Stmt>),
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

#[derive(Clone, Copy)]
enum LoweredFunctionInstantiationKind {
    DirectFunction,
    MarkCoroutineFunction,
}

#[derive(Clone)]
struct LoweredFunctionCaptureValue {
    name: String,
    value_expr: Expr,
}

struct LoweredFunctionInstantiationPreview {
    function_id: usize,
    captures: Vec<LoweredFunctionCaptureValue>,
    kind: LoweredFunctionInstantiationKind,
}

#[derive(Default)]
struct YieldFamilyDetector {
    found: bool,
}

impl Transformer for YieldFamilyDetector {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
            other => walk_stmt(self, other),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Yield(_) | Expr::YieldFrom(_) => {
                self.found = true;
            }
            Expr::Lambda(_)
            | Expr::Generator(_)
            | Expr::ListComp(_)
            | Expr::SetComp(_)
            | Expr::DictComp(_) => {}
            other => walk_expr(self, other),
        }
    }
}

fn function_kind(func: &ast::StmtFunctionDef) -> BlockPyFunctionKind {
    let mut detector = YieldFamilyDetector::default();
    let mut body = suite_ref(&func.body).to_vec();
    detector.visit_body(&mut body);
    match (func.is_async, detector.found) {
        (false, false) => BlockPyFunctionKind::Function,
        (false, true) => BlockPyFunctionKind::Generator,
        (true, false) => BlockPyFunctionKind::Coroutine,
        (true, true) => BlockPyFunctionKind::AsyncGenerator,
    }
}

fn strip_nonlocal_directives(stmts: Vec<Stmt>) -> Vec<Stmt> {
    stmts
        .into_iter()
        .filter(|stmt| !matches!(stmt, Stmt::Global(_) | Stmt::Nonlocal(_)))
        .collect()
}

fn should_strip_nonlocal_for_bb(fn_name: &str) -> bool {
    // Generated helper functions (comprehensions/lambdas/etc.) are prefixed
    // `_dp_fn__dp_...` and currently rely on their existing non-BB lowering
    // behavior for closure propagation. Keep nonlocal directives there.
    !fn_name.starts_with("_dp_fn__dp_")
}

fn collect_deleted_names(stmts: &[Stmt]) -> HashSet<String> {
    let mut names = HashSet::new();
    for stmt in stmts {
        collect_deleted_names_in_stmt(stmt, &mut names);
    }
    names
}

fn collect_deleted_names_in_stmt(stmt: &Stmt, names: &mut HashSet<String>) {
    match stmt {
        Stmt::Delete(delete_stmt) => {
            for target in &delete_stmt.targets {
                collect_deleted_names_in_target(target, names);
            }
        }
        Stmt::If(if_stmt) => {
            for stmt in suite_ref(&if_stmt.body) {
                collect_deleted_names_in_stmt(stmt, names);
            }
            for clause in &if_stmt.elif_else_clauses {
                for stmt in suite_ref(&clause.body) {
                    collect_deleted_names_in_stmt(stmt, names);
                }
            }
        }
        Stmt::While(while_stmt) => {
            for stmt in suite_ref(&while_stmt.body) {
                collect_deleted_names_in_stmt(stmt, names);
            }
            for stmt in suite_ref(&while_stmt.orelse) {
                collect_deleted_names_in_stmt(stmt, names);
            }
        }
        Stmt::For(for_stmt) => {
            for stmt in suite_ref(&for_stmt.body) {
                collect_deleted_names_in_stmt(stmt, names);
            }
            for stmt in suite_ref(&for_stmt.orelse) {
                collect_deleted_names_in_stmt(stmt, names);
            }
        }
        Stmt::Try(try_stmt) => {
            for stmt in suite_ref(&try_stmt.body) {
                collect_deleted_names_in_stmt(stmt, names);
            }
            for handler in &try_stmt.handlers {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                for stmt in suite_ref(&handler.body) {
                    collect_deleted_names_in_stmt(stmt, names);
                }
            }
            for stmt in suite_ref(&try_stmt.orelse) {
                collect_deleted_names_in_stmt(stmt, names);
            }
            for stmt in suite_ref(&try_stmt.finalbody) {
                collect_deleted_names_in_stmt(stmt, names);
            }
        }
        Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
        _ => {}
    }
}

fn collect_deleted_names_in_target(target: &Expr, names: &mut HashSet<String>) {
    match target {
        Expr::Name(name) => {
            names.insert(name.id.to_string());
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                collect_deleted_names_in_target(elt, names);
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                collect_deleted_names_in_target(elt, names);
            }
        }
        Expr::Starred(starred) => collect_deleted_names_in_target(starred.value.as_ref(), names),
        _ => {}
    }
}

struct ReservedTempNamesGuard {
    stack: *mut Vec<HashSet<String>>,
}

impl Drop for ReservedTempNamesGuard {
    fn drop(&mut self) {
        // The guard only exists while function lowering is active and the
        // stack itself lives in the caller, so popping here is safe.
        unsafe {
            (*self.stack).pop();
        }
    }
}

fn try_lower_function_to_blockpy_bundle(
    context: &Context,
    function_identity_by_node: &HashMap<NodeIndex, FunctionIdentity>,
    func: &ast::StmtFunctionDef,
    parent_name: Option<&str>,
    reserved_temp_names_stack: &mut Vec<HashSet<String>>,
    next_block_id: &mut usize,
    next_function_id: &mut usize,
) -> Option<BlockPyFunction<RuffBlockPyPass>> {
    if should_keep_non_lowered_for_annotationlib(func) {
        return None;
    }
    // Keep generated annotation helpers in their lexical scope. BB-lowering
    // and hoisting them out of class/module init can break name resolution
    // for class-local symbols (for example, `T` in `value: T`).
    if is_annotation_helper_name(func.name.id.as_str()) {
        return None;
    }
    let (_, lowered_input_body) = split_docstring(suite_ref(&func.body));
    let lowered_input_body = lowered_input_body.to_vec();
    let lowered_input_body = if should_strip_nonlocal_for_bb(func.name.id.as_str()) {
        strip_nonlocal_directives(lowered_input_body)
    } else {
        lowered_input_body
    };
    let (param_spec, _param_defaults) = collect_param_spec_and_defaults(&func.parameters);
    let param_names = param_spec.names();
    let runtime_input_body = prune_dead_stmt_suffixes(&lowered_input_body);
    let mut outer_scope_names = collect_bound_names(&runtime_input_body);
    outer_scope_names.extend(param_names.iter().cloned());
    let runtime_input_body =
        rewrite_annotation_helper_defs_as_exec_calls(runtime_input_body, &outer_scope_names);
    let mut outer_scope_names = collect_bound_names(&runtime_input_body);
    outer_scope_names.extend(param_names.iter().cloned());
    reserved_temp_names_stack.push(outer_scope_names.clone());
    let _reserved_temp_names_guard = ReservedTempNamesGuard {
        stack: reserved_temp_names_stack,
    };
    let unbound_local_names = if has_dead_stmt_suffixes(&lowered_input_body) {
        always_unbound_local_names(&lowered_input_body, &runtime_input_body, &param_names)
    } else {
        HashSet::new()
    };
    let deleted_names = collect_deleted_names(&runtime_input_body);
    let cell_slots = collect_cell_slots(&runtime_input_body);
    let callable_facts = BlockPyCallableFacts {
        deleted_names,
        unbound_local_names,
        outer_scope_names: outer_scope_names.clone(),
        cell_slots,
    };

    let end_label = next_label(func.name.id.as_str(), next_block_id);
    let identity = resolve_runtime_function_identity(func, function_identity_by_node, parent_name);
    let doc = function_docstring_text(func);
    let main_function_id = take_next_function_id(next_function_id);
    let fn_name = func.name.id.to_string();
    let blockpy_kind = function_kind(func);
    let mut callable_def = build_blockpy_callable_def_from_runtime_input(
        context,
        main_function_id,
        FunctionName::new(
            identity.bind_name.clone(),
            fn_name,
            identity.display_name.clone(),
            identity.qualname.clone(),
        ),
        param_spec,
        &runtime_input_body,
        doc,
        end_label,
        blockpy_kind,
        &callable_facts,
        next_block_id,
        &mut |prefix, next_block_id| {
            next_temp_from_counter(reserved_temp_names_stack, prefix, next_block_id)
        },
    );
    if !callable_facts.deleted_names.is_empty() {
        rewrite_deleted_name_loads(
            &mut callable_def.blocks,
            &callable_facts.deleted_names,
            &callable_facts.unbound_local_names,
        );
    } else if !callable_facts.unbound_local_names.is_empty() {
        rewrite_deleted_name_loads(
            &mut callable_def.blocks,
            &HashSet::new(),
            &callable_facts.unbound_local_names,
        );
    }

    Some(callable_def)
}

fn function_docstring_text(func: &ast::StmtFunctionDef) -> Option<String> {
    let (docstring, _) = split_docstring(suite_ref(&func.body));
    let Some(Stmt::Expr(expr_stmt)) = docstring else {
        return None;
    };
    let Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) = *expr_stmt.value else {
        return None;
    };
    Some(value.to_string())
}

fn split_docstring(body: &Suite) -> (Option<Stmt>, Vec<Stmt>) {
    let mut rest = body.clone();
    let Some(first) = rest.first() else {
        return (None, rest);
    };
    if matches!(
        first,
        Stmt::Expr(ast::StmtExpr { value, .. }) if matches!(value.as_ref(), Expr::StringLiteral(_))
    ) {
        let first_stmt = rest.remove(0);
        return (Some(first_stmt), rest);
    }
    (None, rest)
}

fn has_dead_stmt_suffixes(stmts: &[Stmt]) -> bool {
    let mut terminated = false;
    for stmt in stmts {
        if terminated {
            return true;
        }
        if has_dead_stmt_suffixes_in_stmt(stmt) {
            return true;
        }
        if matches!(
            stmt,
            Stmt::Return(_) | Stmt::Raise(_) | Stmt::Break(_) | Stmt::Continue(_)
        ) {
            terminated = true;
        }
    }
    false
}

fn has_dead_stmt_suffixes_in_stmt(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::If(if_stmt) => {
            has_dead_stmt_suffixes(suite_ref(&if_stmt.body))
                || if_stmt
                    .elif_else_clauses
                    .iter()
                    .any(|clause| has_dead_stmt_suffixes(suite_ref(&clause.body)))
        }
        Stmt::While(while_stmt) => {
            has_dead_stmt_suffixes(suite_ref(&while_stmt.body))
                || has_dead_stmt_suffixes(suite_ref(&while_stmt.orelse))
        }
        Stmt::For(for_stmt) => {
            has_dead_stmt_suffixes(suite_ref(&for_stmt.body))
                || has_dead_stmt_suffixes(suite_ref(&for_stmt.orelse))
        }
        Stmt::Try(try_stmt) => {
            has_dead_stmt_suffixes(suite_ref(&try_stmt.body))
                || try_stmt.handlers.iter().any(|handler| {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    has_dead_stmt_suffixes(suite_ref(&handler.body))
                })
                || has_dead_stmt_suffixes(suite_ref(&try_stmt.orelse))
                || has_dead_stmt_suffixes(suite_ref(&try_stmt.finalbody))
        }
        _ => false,
    }
}

fn prune_dead_stmt_suffixes(stmts: &[Stmt]) -> Vec<Stmt> {
    let mut out = Vec::new();
    for stmt in stmts {
        let mut stmt = stmt.clone();
        prune_dead_stmt_suffixes_in_stmt(&mut stmt);
        let terminates = matches!(
            stmt,
            Stmt::Return(_) | Stmt::Raise(_) | Stmt::Break(_) | Stmt::Continue(_)
        );
        out.push(stmt);
        if terminates {
            break;
        }
    }
    out
}

fn prune_dead_stmt_suffixes_in_stmt(stmt: &mut Stmt) {
    match stmt {
        Stmt::If(if_stmt) => {
            *suite_mut(&mut if_stmt.body) = prune_dead_stmt_suffixes(suite_ref(&if_stmt.body));
            for clause in &mut if_stmt.elif_else_clauses {
                *suite_mut(&mut clause.body) = prune_dead_stmt_suffixes(suite_ref(&clause.body));
            }
        }
        Stmt::While(while_stmt) => {
            *suite_mut(&mut while_stmt.body) =
                prune_dead_stmt_suffixes(suite_ref(&while_stmt.body));
            *suite_mut(&mut while_stmt.orelse) =
                prune_dead_stmt_suffixes(suite_ref(&while_stmt.orelse));
        }
        Stmt::For(for_stmt) => {
            *suite_mut(&mut for_stmt.body) = prune_dead_stmt_suffixes(suite_ref(&for_stmt.body));
            *suite_mut(&mut for_stmt.orelse) =
                prune_dead_stmt_suffixes(suite_ref(&for_stmt.orelse));
        }
        Stmt::Try(try_stmt) => {
            *suite_mut(&mut try_stmt.body) = prune_dead_stmt_suffixes(suite_ref(&try_stmt.body));
            for handler in &mut try_stmt.handlers {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                *suite_mut(&mut handler.body) = prune_dead_stmt_suffixes(suite_ref(&handler.body));
            }
            *suite_mut(&mut try_stmt.orelse) =
                prune_dead_stmt_suffixes(suite_ref(&try_stmt.orelse));
            *suite_mut(&mut try_stmt.finalbody) =
                prune_dead_stmt_suffixes(suite_ref(&try_stmt.finalbody));
        }
        _ => {}
    }
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

fn next_label(fn_name: &str, next_id: &mut usize) -> String {
    let current = *next_id;
    *next_id += 1;
    format!("_dp_bb_{}_{}", sanitize_ident(fn_name), current)
}

fn sanitize_ident(raw: &str) -> String {
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

fn always_unbound_local_names(
    lowered_input_body: &[Stmt],
    runtime_body: &[Stmt],
    param_names: &[String],
) -> HashSet<String> {
    let original_bound_names = collect_bound_names(lowered_input_body);
    let runtime_bound_names = collect_bound_names(runtime_body);
    let explicit_global_or_nonlocal = collect_explicit_global_or_nonlocal_names(lowered_input_body);
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

fn doc_text_to_expr(doc: Option<&str>) -> Expr {
    doc.map(|doc| py_expr!("{doc:literal}", doc = doc))
        .unwrap_or_else(|| py_expr!("None"))
}

struct LoweredFunctionRewriteResult {
    replacement: Vec<Stmt>,
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
fn capture_items_to_expr(captures: &[LoweredFunctionCaptureValue]) -> Expr {
    make_dp_tuple(
        captures
            .iter()
            .map(|capture| {
                make_dp_tuple(vec![
                    py_expr!("{value:literal}", value = capture.name.as_str()),
                    capture.value_expr.clone(),
                ])
            })
            .collect(),
    )
}

fn classify_capture_items(
    entry_liveins: &[String],
    param_names: &HashSet<String>,
    local_cell_slots: &HashSet<String>,
    locally_assigned: &HashSet<String>,
) -> Option<Vec<LoweredFunctionCaptureValue>> {
    let mut captures = Vec::new();
    for entry_name in entry_liveins {
        if param_names.contains(entry_name) {
            continue;
        }
        if entry_name == "_dp_classcell"
            || (entry_name.starts_with("_dp_cell_") && !local_cell_slots.contains(entry_name))
        {
            captures.push(LoweredFunctionCaptureValue {
                name: entry_name.clone(),
                value_expr: name_expr(entry_name.as_str())?,
            });
        } else if !entry_name.starts_with("_dp_") && !locally_assigned.contains(entry_name) {
            captures.push(LoweredFunctionCaptureValue {
                name: entry_name.clone(),
                value_expr: name_expr(entry_name.as_str())?,
            });
        }
    }
    Some(captures)
}

fn build_lowered_function_instantiation_preview(
    callable_def: &BlockPyFunction<RuffBlockPyPass>,
) -> Option<LoweredFunctionInstantiationPreview> {
    let param_names = callable_def.params.names();
    let param_name_set: HashSet<String> = param_names.iter().cloned().collect();
    let entry_liveins = callable_def.entry_liveins();
    let locally_assigned: HashSet<String> = callable_def
        .blocks
        .iter()
        .flat_map(|block| analyze_blockpy_use_def(block).1.into_iter())
        .collect();
    let captures = classify_capture_items(
        &entry_liveins,
        &param_name_set,
        &callable_def.facts.cell_slots,
        &locally_assigned,
    )?;
    Some(LoweredFunctionInstantiationPreview {
        function_id: callable_def.function_id.0,
        captures,
        kind: if callable_def.kind == BlockPyFunctionKind::Coroutine {
            LoweredFunctionInstantiationKind::MarkCoroutineFunction
        } else {
            LoweredFunctionInstantiationKind::DirectFunction
        },
    })
}

struct LoweredFunctionInstantiationData {
    function_id: usize,
    captures: Vec<LoweredFunctionCaptureValue>,
    decorator_exprs: Vec<Expr>,
    param_defaults: Vec<Expr>,
    annotate_fn_expr: Expr,
    kind: LoweredFunctionInstantiationKind,
}

fn build_lowered_function_instantiation_data(
    func: &ast::StmtFunctionDef,
    preview: &LoweredFunctionInstantiationPreview,
    decorator_exprs: Vec<Expr>,
    annotate_fn_expr: Option<Expr>,
) -> LoweredFunctionInstantiationData {
    let (_, param_defaults) = collect_param_spec_and_defaults(&func.parameters);
    LoweredFunctionInstantiationData {
        function_id: preview.function_id,
        captures: preview.captures.clone(),
        decorator_exprs,
        param_defaults,
        annotate_fn_expr: annotate_fn_expr.unwrap_or_else(|| py_expr!("None")),
        kind: preview.kind,
    }
}

fn build_lowered_function_instantiation_expr(data: &LoweredFunctionInstantiationData) -> Expr {
    let capture_expr = capture_items_to_expr(&data.captures);
    let param_defaults_expr = param_defaults_to_expr(&data.param_defaults);
    let function_entry_expr = py_expr!(
        "__dp_make_function({function_id:literal}, {closure:expr}, {param_defaults:expr}, {module_globals:expr}, {annotate_fn:expr})",
        function_id = data.function_id,
        closure = capture_expr.clone(),
        param_defaults = param_defaults_expr.clone(),
        module_globals = py_expr!("__dp_globals()"),
        annotate_fn = data.annotate_fn_expr.clone(),
    );
    let base_function_expr = match data.kind {
        LoweredFunctionInstantiationKind::DirectFunction => function_entry_expr,
        LoweredFunctionInstantiationKind::MarkCoroutineFunction => py_expr!(
            "__dp_mark_coroutine_function({func:expr})",
            func = function_entry_expr,
        ),
    };
    rewrite_stmt::decorator::rewrite_exprs(data.decorator_exprs.clone(), base_function_expr)
}

#[cfg(test)]
mod tests {
    use super::{
        capture_items_to_expr, BlockPyModuleRewriter, FunctionScopeFrame,
        LoweredFunctionCaptureValue,
    };
    use crate::passes::ast_to_ast::body::suite_mut;
    use crate::passes::ast_to_ast::context::Context;
    use crate::passes::ast_to_ast::scope::analyze_module_scope;
    use crate::passes::ast_to_ast::semantic::SemanticAstState;
    use crate::passes::ast_to_ast::Options;
    use crate::passes::function_identity::collect_function_identity_private;
    use ruff_python_ast::Stmt;
    use ruff_python_parser::parse_module;

    #[test]
    fn capture_items_render_as_name_value_pairs() {
        let expr = capture_items_to_expr(&[
            LoweredFunctionCaptureValue {
                name: "x".to_string(),
                value_expr: crate::py_expr!("x"),
            },
            LoweredFunctionCaptureValue {
                name: "y".to_string(),
                value_expr: crate::py_expr!("z"),
            },
        ]);
        assert_eq!(
            crate::ruff_ast_to_string(&expr).trim(),
            "__dp_tuple(__dp_tuple(\"x\", x), __dp_tuple(\"y\", z))"
        );
    }

    #[test]
    fn recursive_local_function_marks_nested_binding_for_cell_sync() {
        let source = concat!(
            "def outer():\n",
            "    def recurse():\n",
            "        return recurse()\n",
            "    return recurse\n",
        );
        let context = Context::new(Options::for_test(), source);
        let mut module = parse_module(source).unwrap().into_syntax().body;
        let module_scope = analyze_module_scope(&mut module);
        let semantic_state = SemanticAstState::from_ruff(&mut module, Some(module_scope.clone()));
        let function_identity_by_node =
            collect_function_identity_private(&mut module, &semantic_state);
        let Stmt::FunctionDef(outer) = &mut module[0] else {
            panic!("expected outer function");
        };
        let outer_scope = module_scope
            .tree
            .scope_for_def(outer)
            .expect("missing outer scope");
        let mut rewriter = BlockPyModuleRewriter {
            context: &context,
            semantic_state: &semantic_state,
            function_identity_by_node,
            next_block_id: 0,
            next_function_id: 0,
            reserved_temp_names_stack: Vec::new(),
            function_scope_stack: vec![FunctionScopeFrame {
                name: "outer".to_string(),
                parent_name: None,
                cell_bindings: outer_scope.local_cell_bindings(),
                entering_module_init: false,
                has_parent_hoisted_scope: false,
                needs_cell_sync: false,
                hoisted_to_parent: Vec::new(),
            }],
            callable_defs: Vec::new(),
        };
        let nested_stmt = suite_mut(&mut outer.body)
            .iter_mut()
            .find(|stmt| matches!(stmt, Stmt::FunctionDef(_)))
            .expect("missing nested function");
        let nested_state = rewriter
            .walk_function_def_with_scope(nested_stmt)
            .expect("expected nested function state");
        assert!(nested_state.needs_cell_sync);
        let Stmt::FunctionDef(nested_func) = nested_stmt else {
            panic!("expected nested function def");
        };
        let replacement = rewriter
            .rewrite_visited_function_def(nested_func, nested_state)
            .expect("expected nested function rewrite");
        let rendered = replacement
            .iter()
            .map(crate::ruff_ast_to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            rendered.contains("__dp_store_cell(_dp_cell_recurse, recurse)"),
            "{rendered}"
        );
    }

    #[test]
    fn walking_outer_function_rewrites_nested_recursive_binding_in_place() {
        let source = concat!(
            "def outer():\n",
            "    def recurse():\n",
            "        return recurse()\n",
            "    return recurse\n",
        );
        let context = Context::new(Options::for_test(), source);
        let mut module = parse_module(source).unwrap().into_syntax().body;
        let module_scope = analyze_module_scope(&mut module);
        let semantic_state = SemanticAstState::from_ruff(&mut module, Some(module_scope.clone()));
        let function_identity_by_node =
            collect_function_identity_private(&mut module, &semantic_state);
        let mut rewriter = BlockPyModuleRewriter {
            context: &context,
            semantic_state: &semantic_state,
            function_identity_by_node,
            next_block_id: 0,
            next_function_id: 0,
            reserved_temp_names_stack: Vec::new(),
            function_scope_stack: Vec::new(),
            callable_defs: Vec::new(),
        };
        let outer_state = rewriter
            .walk_function_def_with_scope(&mut module[0])
            .expect("expected outer function state");
        let Stmt::FunctionDef(outer) = &module[0] else {
            panic!("expected outer function");
        };
        let rendered_body = outer
            .body
            .iter()
            .map(crate::ruff_ast_to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            rendered_body.contains("__dp_store_cell(_dp_cell_recurse, recurse)"),
            "{rendered_body}"
        );
        drop(outer_state);
    }

    #[test]
    fn lowering_outer_function_preserves_recursive_cell_sync_stmt() {
        let source = concat!(
            "def outer():\n",
            "    def recurse():\n",
            "        return recurse()\n",
            "    return recurse\n",
        );
        let context = Context::new(Options::for_test(), source);
        let mut module = parse_module(source).unwrap().into_syntax().body;
        let module_scope = analyze_module_scope(&mut module);
        let semantic_state = SemanticAstState::from_ruff(&mut module, Some(module_scope.clone()));
        let function_identity_by_node =
            collect_function_identity_private(&mut module, &semantic_state);
        let mut rewriter = BlockPyModuleRewriter {
            context: &context,
            semantic_state: &semantic_state,
            function_identity_by_node,
            next_block_id: 0,
            next_function_id: 0,
            reserved_temp_names_stack: Vec::new(),
            function_scope_stack: Vec::new(),
            callable_defs: Vec::new(),
        };
        let outer_state = rewriter
            .walk_function_def_with_scope(&mut module[0])
            .expect("expected outer function state");
        let Stmt::FunctionDef(outer) = &mut module[0] else {
            panic!("expected outer function");
        };
        let _replacement = rewriter
            .rewrite_visited_function_def(outer, outer_state)
            .expect("expected outer function rewrite");
        let outer_callable = rewriter
            .callable_defs
            .iter()
            .find(|callable| callable.names.bind_name == "outer")
            .expect("missing lowered outer callable");
        let rendered =
            crate::block_py::pretty::blockpy_module_to_string(&crate::block_py::BlockPyModule {
                callable_defs: vec![outer_callable.clone()],
            });
        assert!(
            rendered.contains("__dp_store_cell(_dp_cell_recurse, recurse)"),
            "{rendered}"
        );
    }

    #[test]
    fn lowering_recursive_local_function_with_finally_preserves_cell_sync_stmt() {
        let source = concat!(
            "import sys\n",
            "def exercise():\n",
            "    original_limit = sys.getrecursionlimit()\n",
            "    sys.setrecursionlimit(50)\n",
            "    def recurse():\n",
            "        return recurse()\n",
            "    try:\n",
            "        try:\n",
            "            recurse()\n",
            "        except RecursionError:\n",
            "            return True\n",
            "        return False\n",
            "    finally:\n",
            "        sys.setrecursionlimit(original_limit)\n",
        );
        let context = Context::new(Options::for_test(), source);
        let module = parse_module(source).unwrap().into_syntax().body;
        let blockpy = super::rewrite_ast_to_lowered_blockpy_module_plan(&context, module);
        let exercise = blockpy
            .callable_defs
            .iter()
            .find(|callable| callable.names.bind_name == "exercise")
            .expect("missing lowered exercise callable");
        let rendered =
            crate::block_py::pretty::blockpy_module_to_string(&crate::block_py::BlockPyModule {
                callable_defs: vec![exercise.clone()],
            });
        assert!(
            rendered.contains("__dp_store_cell(_dp_cell_recurse, recurse)"),
            "{rendered}"
        );
    }

    #[test]
    fn lowering_recursive_local_function_finally_return_preserves_liveins() {
        let source = concat!(
            "import sys\n",
            "def exercise():\n",
            "    original_limit = sys.getrecursionlimit()\n",
            "    sys.setrecursionlimit(50)\n",
            "    def recurse():\n",
            "        return recurse()\n",
            "    try:\n",
            "        try:\n",
            "            recurse()\n",
            "        except RecursionError:\n",
            "            return True\n",
            "        return False\n",
            "    finally:\n",
            "        sys.setrecursionlimit(original_limit)\n",
        );
        let context = Context::new(Options::for_test(), source);
        let module = parse_module(source).unwrap().into_syntax().body;
        let blockpy = super::rewrite_ast_to_lowered_blockpy_module_plan(&context, module);
        let exercise = blockpy
            .callable_defs
            .iter()
            .find(|callable| callable.names.bind_name == "exercise")
            .expect("missing lowered exercise callable");
        let rendered =
            crate::block_py::pretty::blockpy_module_to_string(&crate::block_py::BlockPyModule {
                callable_defs: vec![exercise.clone()],
            });
        assert!(
            rendered.contains("jump _dp_bb_0(Return, _dp_try_abrupt_payload_5)"),
            "{rendered}"
        );
        assert!(
            !rendered.contains("jump _dp_bb_0(None, Return, _dp_try_abrupt_payload_5)"),
            "{rendered}"
        );
    }
}

pub(crate) fn rewrite_ast_to_lowered_blockpy_module_plan(
    context: &Context,
    module: Suite,
) -> BlockPyModule<RuffBlockPyPass> {
    let mut module = module;
    crate::passes::ast_to_ast::simplify::flatten(&mut module);
    let module_scope = analyze_module_scope(&mut module);
    let semantic_state = SemanticAstState::from_scope_tree(&mut module, module_scope);
    rewrite_ast_to_lowered_blockpy_module_plan_with_module(
        context,
        &mut module,
        &semantic_state,
        &semantic_state,
    )
}

pub(crate) fn rewrite_ast_to_lowered_blockpy_module_plan_with_module(
    context: &Context,
    module: &mut Suite,
    semantic_state: &SemanticAstState,
    function_identity_state: &SemanticAstState,
) -> BlockPyModule<RuffBlockPyPass> {
    crate::passes::ast_to_ast::simplify::flatten(module);
    let function_identity_by_node =
        collect_function_identity_private(module, function_identity_state);
    let mut rewriter = BlockPyModuleRewriter {
        context,
        semantic_state,
        function_identity_by_node,
        next_block_id: 0,
        next_function_id: 0,
        reserved_temp_names_stack: Vec::new(),
        function_scope_stack: Vec::new(),
        callable_defs: Vec::new(),
    };
    rewriter.visit_body(module);
    BlockPyModule {
        callable_defs: rewriter.callable_defs,
    }
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
    doc: Option<String>,
    decorator_exprs: Vec<Expr>,
) -> Stmt {
    let updated = py_expr!(
        "__dp_update_fn({name:id}, {qualname:literal}, {display_name:literal}, {doc:expr})",
        name = local_name,
        qualname = qualname,
        display_name = display_name,
        doc = doc_text_to_expr(doc.as_deref()),
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
    doc: Option<String>,
) -> Option<Vec<Stmt>> {
    match binding_plan {
        NonLoweredFunctionBindingPlan::LeaveLocal => None,
        NonLoweredFunctionBindingPlan::CellSyncOnly => Some(vec![build_cell_sync_stmt(bind_name)]),
        NonLoweredFunctionBindingPlan::Rebind { target } => {
            let local_name = if let Some(local_name) = fresh_local_name {
                func.name.id = Name::new(local_name.as_str());
                local_name
            } else {
                func.name.id.to_string()
            };
            let decorator_exprs =
                rewrite_stmt::decorator::into_exprs(std::mem::take(&mut func.decorator_list));
            Some(vec![build_updated_function_binding_stmt(
                target,
                bind_name,
                local_name.as_str(),
                qualname,
                display_name,
                doc,
                decorator_exprs,
            )])
        }
    }
}

fn plan_lowered_function_placement(
    bind_name: &str,
    entering_module_init: bool,
    has_parent_hoisted_scope: bool,
    function_hoisted: Vec<Stmt>,
    binding_stmt: Vec<Stmt>,
) -> LoweredFunctionPlacementPlan {
    let keep_local_blocks = !entering_module_init
        && has_parent_hoisted_scope
        && (bind_name.starts_with("_dp_class_ns_") || bind_name.starts_with("_dp_define_class_"));

    if entering_module_init || keep_local_blocks || !has_parent_hoisted_scope {
        let mut body = function_hoisted;
        body.extend(binding_stmt);
        LoweredFunctionPlacementPlan::ReplaceWith(body)
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
    binding_stmt: Option<Vec<Stmt>>,
) -> NonLoweredFunctionPlacementPlan {
    if let Some(binding_stmt) = binding_stmt {
        let mut body = function_hoisted;
        body.push(function_stmt);
        body.extend(binding_stmt);
        NonLoweredFunctionPlacementPlan::ReplaceWith(body)
    } else if !function_hoisted.is_empty() {
        NonLoweredFunctionPlacementPlan::PrependBody(function_hoisted)
    } else {
        NonLoweredFunctionPlacementPlan::LeaveInPlace
    }
}

fn apply_lowered_function_placement(
    parent_hoisted: Option<&mut Vec<Stmt>>,
    plan: LoweredFunctionPlacementPlan,
) -> Vec<Stmt> {
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
) -> Option<Vec<Stmt>> {
    match plan {
        NonLoweredFunctionPlacementPlan::ReplaceWith(replacement) => Some(replacement),
        NonLoweredFunctionPlacementPlan::PrependBody(function_hoisted) => {
            let mut new_body = function_hoisted;
            new_body.extend(take_suite(&mut func.body));
            *suite_mut(&mut func.body) = new_body;
            None
        }
        NonLoweredFunctionPlacementPlan::LeaveInPlace => None,
    }
}

fn build_lowered_function_binding_stmt(
    bind_name: &str,
    value: Expr,
    binding_plan: LoweredFunctionBindingPlan,
) -> Vec<Stmt> {
    match binding_plan.target {
        BindingTarget::Local => {
            let assign_stmt = py_stmt!("{name:id} = {value:expr}", name = bind_name, value = value);
            if binding_plan.needs_cell_sync {
                vec![assign_stmt, build_cell_sync_stmt(bind_name)]
            } else {
                vec![assign_stmt]
            }
        }
        BindingTarget::ModuleGlobal | BindingTarget::ClassNamespace => {
            vec![build_binding_stmt(binding_plan.target, bind_name, value)]
        }
    }
}

fn build_lowered_function_instantiation_stmt(
    func: &ast::StmtFunctionDef,
    preview: &LoweredFunctionInstantiationPreview,
    instantiation_plan: &LoweredFunctionInstantiationPlan,
) -> Vec<Stmt> {
    let bind_name = instantiation_plan.identity.bind_name.as_str();
    let annotate_helper = build_lowered_annotation_helper_binding(func, bind_name);
    let annotate_fn_expr = annotate_helper
        .as_ref()
        .map(|(_, annotate_fn_expr)| annotate_fn_expr.clone());
    let instantiation_data = build_lowered_function_instantiation_data(
        func,
        preview,
        rewrite_stmt::decorator::collect_exprs(&func.decorator_list),
        annotate_fn_expr,
    );
    let decorated = build_lowered_function_instantiation_expr(&instantiation_data);
    let binding_stmt =
        build_lowered_function_binding_stmt(bind_name, decorated, instantiation_plan.binding);
    let mut stmts = Vec::new();
    if let Some((helper_stmt, _)) = annotate_helper {
        stmts.push(helper_stmt);
    }
    stmts.extend(binding_stmt);
    stmts
}

fn rewrite_lowered_function_instantiation_stmt(
    parent_hoisted: Option<&mut Vec<Stmt>>,
    func: &ast::StmtFunctionDef,
    preview: &LoweredFunctionInstantiationPreview,
    instantiation_plan: &LoweredFunctionInstantiationPlan,
    entering_module_init: bool,
    has_parent_hoisted_scope: bool,
    function_hoisted: Vec<Stmt>,
) -> Option<LoweredFunctionRewriteResult> {
    let binding_stmt = build_lowered_function_instantiation_stmt(func, preview, instantiation_plan);
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
    preview: &LoweredFunctionInstantiationPreview,
    function_identity_by_node: &HashMap<NodeIndex, FunctionIdentity>,
    current_parent: Option<&str>,
    needs_cell_sync: bool,
    entering_module_init: bool,
    has_parent_hoisted_scope: bool,
    function_hoisted: Vec<Stmt>,
) -> Option<LoweredFunctionRewriteResult> {
    let instantiation_plan = plan_lowered_function_instantiation(
        func,
        function_identity_by_node,
        current_parent,
        needs_cell_sync,
    );
    let rewrite = rewrite_lowered_function_instantiation_stmt(
        parent_hoisted,
        func,
        preview,
        &instantiation_plan,
        entering_module_init,
        has_parent_hoisted_scope,
        function_hoisted,
    )?;
    Some(rewrite)
}

fn rewrite_non_lowered_function_instantiation(
    func: &mut ast::StmtFunctionDef,
    instantiation_plan: NonLoweredFunctionInstantiationPlan,
    function_hoisted: Vec<Stmt>,
    doc: Option<String>,
    mut next_temp: impl FnMut() -> String,
) -> Option<Vec<Stmt>> {
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
    doc: Option<String>,
    next_temp: impl FnMut() -> String,
) -> Option<Vec<Stmt>> {
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
    parent_hoisted: Option<&mut Vec<Stmt>>,
    function_identity_by_node: &HashMap<NodeIndex, FunctionIdentity>,
    func: &mut ast::StmtFunctionDef,
    current_parent: Option<&str>,
    needs_cell_sync: bool,
    entering_module_init: bool,
    has_parent_hoisted_scope: bool,
    function_hoisted: Vec<Stmt>,
    reserved_temp_names_stack: &mut Vec<HashSet<String>>,
    next_block_id: &mut usize,
    next_function_id: &mut usize,
    callable_defs: &mut Vec<BlockPyFunction<RuffBlockPyPass>>,
) -> Option<Vec<Stmt>> {
    let doc = function_docstring_text(func);
    if let Some(lowered_plan) = try_lower_function_to_blockpy_bundle(
        context,
        function_identity_by_node,
        func,
        current_parent,
        reserved_temp_names_stack,
        next_block_id,
        next_function_id,
    ) {
        let preview = build_lowered_function_instantiation_preview(&lowered_plan)
            .expect("failed to build BB function instantiation preview");
        let rewrite = plan_and_rewrite_lowered_function_instantiation(
            parent_hoisted,
            func,
            &preview,
            function_identity_by_node,
            current_parent,
            needs_cell_sync,
            entering_module_init,
            has_parent_hoisted_scope,
            function_hoisted,
        )
        .expect("failed to build BB function binding");
        callable_defs.push(lowered_plan);
        return Some(rewrite.replacement);
    }

    plan_and_rewrite_non_lowered_function_instantiation(
        context,
        func,
        function_identity_by_node,
        current_parent,
        needs_cell_sync,
        function_hoisted,
        doc,
        || next_temp_from_counter(reserved_temp_names_stack, "fn_local", next_block_id),
    )
}

impl BlockPyModuleRewriter<'_> {
    fn walk_function_def_with_scope(&mut self, stmt: &mut Stmt) -> Option<FunctionScopeFrame> {
        let Stmt::FunctionDef(func) = stmt else {
            return None;
        };
        let fn_name = func.name.id.to_string();
        let bind_name = func.name.id.to_string();
        let function_scope = self.semantic_state.function_scope(func);
        let parent_name = self
            .function_scope_stack
            .last()
            .map(|frame| frame.name.clone());
        let entering_module_init = is_module_init_temp_name(fn_name.as_str());
        let has_parent_hoisted_scope = !self.function_scope_stack.is_empty();
        let cell_bindings = function_scope
            .as_ref()
            .map(|scope| scope.local_cell_bindings())
            .unwrap_or_default();
        let needs_cell_sync = self
            .function_scope_stack
            .last()
            .map(|frame| frame.cell_bindings.contains(bind_name.as_str()))
            .unwrap_or(false);
        self.function_scope_stack.push(FunctionScopeFrame {
            name: fn_name,
            parent_name,
            cell_bindings,
            entering_module_init,
            has_parent_hoisted_scope,
            needs_cell_sync,
            hoisted_to_parent: Vec::new(),
        });
        walk_stmt(self, stmt);
        self.function_scope_stack.pop()
    }

    fn rewrite_visited_function_def(
        &mut self,
        func: &mut ast::StmtFunctionDef,
        state: FunctionScopeFrame,
    ) -> Option<Vec<Stmt>> {
        let parent_hoisted = self
            .function_scope_stack
            .last_mut()
            .map(|parent_frame| &mut parent_frame.hoisted_to_parent);
        rewrite_function_def_stmt_via_blockpy(
            self.context,
            parent_hoisted,
            &self.function_identity_by_node,
            func,
            state.parent_name.as_deref(),
            state.needs_cell_sync,
            state.entering_module_init,
            state.has_parent_hoisted_scope,
            state.hoisted_to_parent,
            &mut self.reserved_temp_names_stack,
            &mut self.next_block_id,
            &mut self.next_function_id,
            &mut self.callable_defs,
        )
    }
}

impl Transformer for BlockPyModuleRewriter<'_> {
    fn visit_body(&mut self, body: &mut Suite) {
        let mut rewritten = Vec::with_capacity(body.len());
        for stmt in std::mem::take(body) {
            let mut stmt = stmt;
            if matches!(stmt, Stmt::FunctionDef(_)) {
                let Some(state) = self.walk_function_def_with_scope(&mut stmt) else {
                    rewritten.push(stmt);
                    continue;
                };
                if let Stmt::FunctionDef(func) = &mut stmt {
                    if let Some(replacement) = self.rewrite_visited_function_def(func, state) {
                        rewritten.extend(replacement);
                        continue;
                    }
                }
                rewritten.push(stmt);
                continue;
            }

            self.visit_stmt(&mut stmt);
            rewritten.push(stmt);
        }
        *body = rewritten;
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);
    }
}
