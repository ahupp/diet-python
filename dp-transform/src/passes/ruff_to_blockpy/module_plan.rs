use crate::block_py::dataflow::{analyze_blockpy_use_def, loaded_names_in_blockpy_block};
use crate::block_py::param_specs::{collect_param_spec_and_defaults, param_defaults_to_expr};
use crate::block_py::state::collect_cell_slots;
use crate::block_py::{BindingTarget, BlockPyBindingKind, BlockPyCellBindingKind};
use crate::block_py::{
    BlockPyCallableFacts, BlockPyCallableScopeKind, BlockPyCallableSemanticInfo, BlockPyFunction,
    BlockPyFunctionKind, BlockPyModule, FunctionName,
};
use crate::passes::annotation_export::{
    build_lowered_annotation_helper_binding, rewrite_annotation_helper_defs_as_exec_calls,
};
use crate::passes::ast_symbol_analysis::{
    collect_bound_names, collect_explicit_global_or_nonlocal_names, collect_loaded_names,
};
use crate::passes::ast_to_ast::body::{suite_mut, suite_ref, Suite};
use crate::passes::ast_to_ast::context::Context;
use crate::passes::ast_to_ast::expr_utils::{make_dp_tuple, name_expr};
use crate::passes::ast_to_ast::rewrite_stmt;
use crate::passes::ast_to_ast::scope_helpers::{cell_name, is_internal_symbol};
use crate::passes::ast_to_ast::semantic::{
    SemanticAstState, SemanticBindingKind, SemanticScope, SemanticScopeKind,
};
use crate::passes::RuffBlockPyPass;

use crate::passes::function_identity::{
    collect_function_identity_private, resolve_runtime_function_identity, FunctionIdentity,
};
use crate::passes::ruff_to_blockpy::recompute_semantic_blockpy_closure_layout;
use crate::transformer::{walk_expr, walk_stmt, Transformer};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, NodeIndex, Stmt};
use std::collections::{HashMap, HashSet};

use super::{
    build_blockpy_callable_def_from_runtime_input, rewrite_deleted_name_loads,
    take_next_function_id, NameGen,
};

struct FunctionScopeFrame {
    name: String,
    parent_name: Option<String>,
    callable_semantic: BlockPyCallableSemanticInfo,
    hoisted_to_parent: Vec<Stmt>,
}

struct BlockPyModuleRewriter<'a> {
    context: &'a Context,
    semantic_state: &'a SemanticAstState,
    function_identity_by_node: HashMap<NodeIndex, FunctionIdentity>,
    next_function_id: usize,
    function_scope_stack: Vec<FunctionScopeFrame>,
    callable_defs: Vec<BlockPyFunction<RuffBlockPyPass>>,
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

#[derive(Default)]
struct YieldFamilyDetector {
    found: bool,
}

fn callable_semantic_info(
    function_scope: Option<&SemanticScope>,
    body: &[Stmt],
) -> BlockPyCallableSemanticInfo {
    let Some(function_scope) = function_scope else {
        return BlockPyCallableSemanticInfo::default();
    };
    let local_cell_bindings = function_scope.local_cell_bindings();
    let mut bindings = function_scope
        .bindings()
        .into_iter()
        .map(|(name, binding)| {
            (
                name.clone(),
                blockpy_binding_kind_for_name(
                    name.as_str(),
                    binding,
                    &local_cell_bindings,
                    function_scope.has_local_def(name.as_str()),
                ),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut relevant_names = collect_bound_names(body);
    relevant_names.extend(collect_loaded_names(body));
    for name in relevant_names {
        bindings.entry(name.clone()).or_insert_with(|| {
            blockpy_binding_kind_for_name(
                name.as_str(),
                function_scope.resolved_load_binding(name.as_str()),
                &local_cell_bindings,
                function_scope.has_local_def(name.as_str()),
            )
        });
    }
    BlockPyCallableSemanticInfo {
        scope_kind: match function_scope.kind() {
            SemanticScopeKind::Function => BlockPyCallableScopeKind::Function,
            SemanticScopeKind::Class => BlockPyCallableScopeKind::Class,
            SemanticScopeKind::Module => BlockPyCallableScopeKind::Module,
        },
        bindings,
    }
}

fn blockpy_binding_kind_for_name(
    name: &str,
    binding: SemanticBindingKind,
    local_cell_bindings: &HashSet<String>,
    has_local_def: bool,
) -> BlockPyBindingKind {
    match binding {
        SemanticBindingKind::Local if local_cell_bindings.contains(name) => {
            BlockPyBindingKind::Cell(BlockPyCellBindingKind::Owner)
        }
        SemanticBindingKind::Local => BlockPyBindingKind::Local,
        SemanticBindingKind::Nonlocal if has_local_def && local_cell_bindings.contains(name) => {
            BlockPyBindingKind::Cell(BlockPyCellBindingKind::Owner)
        }
        SemanticBindingKind::Nonlocal => BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture),
        SemanticBindingKind::Global => BlockPyBindingKind::Global,
    }
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

fn try_lower_function_to_blockpy_bundle(
    context: &Context,
    function_identity_by_node: &HashMap<NodeIndex, FunctionIdentity>,
    func: &ast::StmtFunctionDef,
    parent_name: Option<&str>,
    callable_semantic: &BlockPyCallableSemanticInfo,
    name_gen: &NameGen,
) -> BlockPyFunction<RuffBlockPyPass> {
    let (_, lowered_input_body) = split_docstring(suite_ref(&func.body));
    let lowered_input_body = lowered_input_body.to_vec();
    let (param_spec, _param_defaults) = collect_param_spec_and_defaults(&func.parameters);
    let param_names = param_spec.names();
    let runtime_input_body = prune_dead_stmt_suffixes(&lowered_input_body);
    let mut outer_scope_names = collect_bound_names(&runtime_input_body);
    outer_scope_names.extend(param_names.iter().cloned());
    let runtime_input_body =
        rewrite_annotation_helper_defs_as_exec_calls(runtime_input_body, &outer_scope_names);
    let mut outer_scope_names = collect_bound_names(&runtime_input_body);
    outer_scope_names.extend(param_names.iter().cloned());
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
        capture_storage_names: HashSet::new(),
    };

    let end_label = name_gen.next_block_name();
    let identity = resolve_runtime_function_identity(func, function_identity_by_node, parent_name);
    let doc = function_docstring_text(func);
    let fn_name = func.name.id.to_string();
    let blockpy_kind = function_kind(func);
    let mut callable_def = build_blockpy_callable_def_from_runtime_input(
        context,
        name_gen,
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
        callable_semantic,
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
    callable_def
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
    used_names: &HashSet<String>,
    defined_names: &HashSet<String>,
    param_names: &HashSet<String>,
    local_cell_slots: &HashSet<String>,
    callable_semantic: &BlockPyCallableSemanticInfo,
) -> Vec<LoweredFunctionCaptureValue> {
    let mut captures = Vec::new();
    let mut referenced_names = used_names
        .iter()
        .chain(defined_names.iter())
        .cloned()
        .collect::<Vec<_>>();
    referenced_names.sort();
    referenced_names.dedup();
    for used_name in referenced_names {
        if param_names.contains(used_name.as_str()) {
            continue;
        }
        if used_name == "_dp_classcell" {
            if param_names.contains("_dp_classcell_arg")
                || defined_names.contains(used_name.as_str())
            {
                continue;
            }
            if local_cell_slots.contains(cell_name("_dp_classcell").as_str()) {
                continue;
            }
            captures.push(LoweredFunctionCaptureValue {
                name: used_name.clone(),
                value_expr: name_expr(used_name.as_str())
                    .expect("capture name should always parse as an expression"),
            });
        } else if used_name.starts_with("_dp_cell_")
            && !local_cell_slots.contains(used_name.as_str())
        {
            captures.push(LoweredFunctionCaptureValue {
                name: used_name.clone(),
                value_expr: name_expr(used_name.as_str())
                    .expect("capture name should always parse as an expression"),
            });
        } else if callable_semantic.resolved_load_binding_kind(used_name.as_str())
            == BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture)
        {
            let capture_name = cell_name(used_name.as_str());
            if local_cell_slots.contains(capture_name.as_str())
                || defined_names.contains(capture_name.as_str())
            {
                continue;
            }
            captures.push(LoweredFunctionCaptureValue {
                name: capture_name.clone(),
                value_expr: name_expr(capture_name.as_str())
                    .expect("capture name should always parse as an expression"),
            });
        }
    }
    captures
}

fn build_lowered_function_instantiation_expr(
    function_id: usize,
    captures: &[LoweredFunctionCaptureValue],
    decorator_exprs: Vec<Expr>,
    param_defaults: &[Expr],
    annotate_fn_expr: Expr,
    kind: LoweredFunctionInstantiationKind,
) -> Expr {
    let capture_expr = capture_items_to_expr(captures);
    let param_defaults_expr = param_defaults_to_expr(param_defaults);
    let function_entry_expr = py_expr!(
        "__dp_make_function({function_id:literal}, {closure:expr}, {param_defaults:expr}, {module_globals:expr}, {annotate_fn:expr})",
        function_id = function_id,
        closure = capture_expr.clone(),
        param_defaults = param_defaults_expr.clone(),
        module_globals = py_expr!("__dp_globals()"),
        annotate_fn = annotate_fn_expr.clone(),
    );
    let base_function_expr = match kind {
        LoweredFunctionInstantiationKind::DirectFunction => function_entry_expr,
        LoweredFunctionInstantiationKind::MarkCoroutineFunction => py_expr!(
            "__dp_mark_coroutine_function({func:expr})",
            func = function_entry_expr,
        ),
    };
    rewrite_stmt::decorator::rewrite_exprs(decorator_exprs, base_function_expr)
}

#[cfg(test)]
mod tests {
    use super::{
        callable_semantic_info, capture_items_to_expr,
        rewrite_ast_to_lowered_blockpy_module_plan_with_module, BlockPyModuleRewriter,
        FunctionScopeFrame, LoweredFunctionCaptureValue,
    };
    use crate::block_py::BlockPyModule;
    use crate::passes::ast_to_ast::body::suite_mut;
    use crate::passes::ast_to_ast::context::Context;
    use crate::passes::ast_to_ast::semantic::SemanticAstState;
    use crate::passes::ast_to_ast::Options;
    use crate::passes::function_identity::{collect_function_identity_private, FunctionIdentity};
    use crate::passes::RuffBlockPyPass;
    use crate::transformer::{walk_stmt, Transformer};
    use ruff_python_ast::{NodeIndex, Stmt};
    use ruff_python_parser::parse_module;
    use std::collections::{HashMap, HashSet};

    fn lower_test_module_plan(
        context: &Context,
        mut module: Vec<Stmt>,
    ) -> BlockPyModule<RuffBlockPyPass> {
        crate::passes::ast_to_ast::simplify::flatten(&mut module);
        let mut semantic_state = SemanticAstState::from_ruff(&mut module);
        if !module.iter().any(
            |stmt| matches!(stmt, Stmt::FunctionDef(func) if func.name.id.as_str() == "_dp_module_init"),
        ) {
            crate::driver::wrap_module_init(&mut semantic_state, &mut module);
        }
        rewrite_ast_to_lowered_blockpy_module_plan_with_module(
            context,
            &mut module,
            &semantic_state,
            &semantic_state,
        )
    }

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
    fn recursive_local_function_bindings_are_cell_owned_in_parent_scope() {
        let source = concat!(
            "def outer():\n",
            "    def recurse():\n",
            "        return recurse()\n",
            "    return recurse\n",
        );
        let context = Context::new(Options::for_test(), source);
        let mut module = parse_module(source).unwrap().into_syntax().body;
        let semantic_state = SemanticAstState::from_ruff(&mut module);
        let function_identity_by_node =
            collect_function_identity_private(&mut module, &semantic_state);
        let Stmt::FunctionDef(outer) = &mut module[0] else {
            panic!("expected outer function");
        };
        let outer_scope = semantic_state
            .function_scope(outer)
            .expect("missing outer scope");
        let mut rewriter = BlockPyModuleRewriter {
            context: &context,
            semantic_state: &semantic_state,
            function_identity_by_node,
            next_function_id: 0,
            function_scope_stack: vec![FunctionScopeFrame {
                name: "outer".to_string(),
                parent_name: None,
                callable_semantic: callable_semantic_info(
                    Some(&outer_scope),
                    crate::passes::ast_to_ast::body::suite_ref(&outer.body),
                ),
                hoisted_to_parent: Vec::new(),
            }],
            callable_defs: Vec::new(),
        };
        let nested_stmt = suite_mut(&mut outer.body)
            .iter_mut()
            .find(|stmt| matches!(stmt, Stmt::FunctionDef(_)))
            .expect("missing nested function");
        let Stmt::FunctionDef(nested_func) = nested_stmt else {
            panic!("expected nested function def");
        };
        let nested_state = rewriter.walk_function_def_with_scope(nested_func);
        assert_eq!(
            rewriter
                .function_scope_stack
                .last()
                .expect("missing outer function frame")
                .callable_semantic
                .binding_kind("recurse"),
            Some(crate::block_py::BlockPyBindingKind::Cell(
                crate::block_py::BlockPyCellBindingKind::Owner
            ))
        );
        let replacement = rewriter.rewrite_visited_function_def(nested_func, nested_state);
        let rendered = replacement
            .iter()
            .map(crate::ruff_ast_to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !rendered.contains("__dp_store_cell(_dp_cell_recurse, recurse)"),
            "{rendered}"
        );
    }

    #[test]
    fn callable_semantic_bindings_match_function_identity_targets() {
        struct Checker<'a> {
            semantic_state: &'a SemanticAstState,
            identity_by_node: &'a HashMap<NodeIndex, FunctionIdentity>,
            scope_stack: Vec<crate::passes::ast_to_ast::semantic::SemanticScope>,
        }

        impl Transformer for Checker<'_> {
            fn visit_stmt(&mut self, stmt: &mut Stmt) {
                match stmt {
                    Stmt::FunctionDef(func) => {
                        let identity = self
                            .identity_by_node
                            .get(&func.node_index.load())
                            .expect("missing function identity");
                        let parent_scope = self.scope_stack.last().expect("missing parent scope");
                        let parent_semantic = callable_semantic_info(Some(parent_scope), &[]);
                        assert_eq!(
                            parent_semantic.binding_target_for_name(identity.bind_name.as_str()),
                            identity.binding_target,
                            "{}",
                            identity.bind_name
                        );
                        if let Some(function_scope) = self.semantic_state.function_scope(func) {
                            self.scope_stack.push(function_scope);
                            walk_stmt(self, stmt);
                            self.scope_stack.pop();
                            return;
                        }
                        walk_stmt(self, stmt);
                    }
                    Stmt::ClassDef(class_def) => {
                        let parent_scope = self
                            .scope_stack
                            .last()
                            .expect("missing parent scope")
                            .clone();
                        if let Some(class_scope) = parent_scope.child_scope_for_class(class_def) {
                            self.scope_stack.push(class_scope);
                            walk_stmt(self, stmt);
                            self.scope_stack.pop();
                            return;
                        }
                        walk_stmt(self, stmt);
                    }
                    _ => walk_stmt(self, stmt),
                }
            }
        }

        let source = concat!(
            "def outer():\n",
            "    def local():\n",
            "        return 1\n",
            "    global exported\n",
            "    def exported():\n",
            "        return 2\n",
            "    class C:\n",
            "        def method(self):\n",
            "            return local()\n",
            "    return local, exported, C\n",
        );
        let mut module = parse_module(source).unwrap().into_syntax().body;
        let mut semantic_state = SemanticAstState::from_ruff(&mut module);
        crate::driver::wrap_module_init(&mut semantic_state, &mut module);
        let identity_by_node = collect_function_identity_private(&mut module, &semantic_state);
        let Stmt::FunctionDef(module_init) = &mut module[0] else {
            panic!("expected _dp_module_init");
        };
        let module_init_scope = semantic_state
            .function_scope(module_init)
            .expect("missing module init scope");
        let mut checker = Checker {
            semantic_state: &semantic_state,
            identity_by_node: &identity_by_node,
            scope_stack: vec![module_init_scope],
        };
        checker.visit_body(&mut module_init.body);
    }

    #[test]
    fn callable_semantic_info_resolves_implicit_global_loads_in_body() {
        let source = concat!(
            "def outer(scale):\n",
            "    factor = scale\n",
            "    def inner(x):\n",
            "        try:\n",
            "            return x + factor\n",
            "        except Exception as exc:\n",
            "            return len(str(exc))\n",
            "    return inner\n",
        );
        let mut module = parse_module(source).unwrap().into_syntax().body;
        let semantic_state = SemanticAstState::from_ruff(&mut module);
        let Stmt::FunctionDef(outer) = &module[0] else {
            panic!("expected outer function");
        };
        let inner = crate::passes::ast_to_ast::body::suite_ref(&outer.body)
            .iter()
            .find_map(|stmt| match stmt {
                Stmt::FunctionDef(func) if func.name.id.as_str() == "inner" => Some(func),
                _ => None,
            })
            .expect("missing inner");
        let inner_scope = semantic_state
            .function_scope(inner)
            .expect("missing inner scope");
        let semantic = callable_semantic_info(
            Some(&inner_scope),
            crate::passes::ast_to_ast::body::suite_ref(&inner.body),
        );

        assert_eq!(
            semantic.binding_kind("factor"),
            Some(crate::block_py::BlockPyBindingKind::Cell(
                crate::block_py::BlockPyCellBindingKind::Capture
            ))
        );
        assert_eq!(
            semantic.binding_kind("x"),
            Some(crate::block_py::BlockPyBindingKind::Local)
        );
        assert_eq!(
            semantic.binding_kind("Exception"),
            Some(crate::block_py::BlockPyBindingKind::Global)
        );
        assert_eq!(
            semantic.binding_kind("len"),
            Some(crate::block_py::BlockPyBindingKind::Global)
        );
        assert_eq!(
            semantic.binding_kind("str"),
            Some(crate::block_py::BlockPyBindingKind::Global)
        );
    }

    #[test]
    fn lowering_recursive_local_function_with_finally_keeps_plain_binding_before_name_binding() {
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
        let blockpy = lower_test_module_plan(&context, module);
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
            rendered.contains("recurse = __dp_make_function"),
            "{rendered}"
        );
        assert!(
            !rendered.contains("__dp_store_cell(_dp_cell_recurse, recurse)"),
            "{rendered}"
        );
    }

    #[test]
    fn lowering_recursive_local_function_treats_recurse_cell_as_local_state() {
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
        let blockpy = lower_test_module_plan(&context, module);
        let exercise = blockpy
            .callable_defs
            .iter()
            .find(|callable| callable.names.bind_name == "exercise")
            .expect("missing lowered exercise callable");
        assert!(
            exercise.semantic.binding_kind("recurse")
                == Some(crate::block_py::BlockPyBindingKind::Cell(
                    crate::block_py::BlockPyCellBindingKind::Owner
                )),
            "semantic_bindings={:?} facts_cell_slots={:?}",
            exercise.semantic.bindings,
            exercise.facts.cell_slots,
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
        let blockpy = lower_test_module_plan(&context, module);
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
            rendered.contains("jump ") && rendered.contains("(Return, _dp_try_abrupt_payload_"),
            "{rendered}"
        );
        assert!(
            !rendered.contains("(None, Return, _dp_try_abrupt_payload_"),
            "{rendered}"
        );
    }

    #[test]
    fn lowering_nonlocal_inner_captures_outer_cell() {
        let source = concat!(
            "def outer():\n",
            "    x = 5\n",
            "    def inner():\n",
            "        nonlocal x\n",
            "        x = 2\n",
            "        return x\n",
            "    return inner()\n",
        );
        let context = Context::new(Options::for_test(), source);
        let module = parse_module(source).unwrap().into_syntax().body;
        let blockpy = lower_test_module_plan(&context, module);
        let inner = blockpy
            .callable_defs
            .iter()
            .find(|callable| callable.names.bind_name == "inner")
            .expect("missing lowered inner callable");
        let outer = blockpy
            .callable_defs
            .iter()
            .find(|callable| callable.names.bind_name == "outer")
            .expect("missing lowered outer callable");
        assert!(
            inner
                .closure_layout()
                .as_ref()
                .expect("inner should have closure layout")
                .freevars
                .iter()
                .any(|slot| slot.storage_name == "_dp_cell_x"),
            "{:?}",
            inner.closure_layout()
        );
        let rendered =
            crate::block_py::pretty::blockpy_module_to_string(&crate::block_py::BlockPyModule {
                callable_defs: vec![outer.clone()],
            });
        assert!(
            rendered.contains(
                "__dp_make_function(0, __dp_tuple(__dp_tuple(\"_dp_cell_x\", _dp_cell_x))"
            ),
            "{rendered}"
        );
    }
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
        next_function_id: 0,
        function_scope_stack: Vec::new(),
        callable_defs: Vec::new(),
    };
    let module_init = BlockPyModuleRewriter::root_module_init_stmt(module);
    rewriter.lower_root_function_def(module_init);
    BlockPyModule {
        callable_defs: rewriter.callable_defs,
    }
}

fn build_binding_stmt(target: BindingTarget, bind_name: &str, value: Expr) -> Stmt {
    match target {
        BindingTarget::Local => {
            py_stmt!("{name:id} = {value:expr}", name = bind_name, value = value,)
        }
        BindingTarget::ModuleGlobal => {
            panic!("module-global binding should be lowered in the name_binding pass")
        }
        BindingTarget::ClassNamespace => py_stmt!(
            "__dp_setitem(_dp_class_ns, {name:literal}, {value:expr})",
            name = bind_name,
            value = value,
        ),
    }
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

fn build_lowered_function_binding_stmt(
    bind_name: &str,
    value: Expr,
    target: BindingTarget,
) -> Vec<Stmt> {
    match target {
        BindingTarget::Local => {
            vec![py_stmt!(
                "{name:id} = {value:expr}",
                name = bind_name,
                value = value
            )]
        }
        BindingTarget::ModuleGlobal => {
            vec![py_stmt!(
                "{name:id} = {value:expr}",
                name = bind_name,
                value = value
            )]
        }
        BindingTarget::ClassNamespace => {
            vec![build_binding_stmt(target, bind_name, value)]
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn rewrite_function_def_stmt_via_blockpy(
    context: &Context,
    parent_hoisted: &mut Vec<Stmt>,
    parent_semantic: &BlockPyCallableSemanticInfo,
    function_identity_by_node: &HashMap<NodeIndex, FunctionIdentity>,
    func: &mut ast::StmtFunctionDef,
    current_parent: Option<&str>,
    callable_semantic: &BlockPyCallableSemanticInfo,
    function_hoisted: Vec<Stmt>,
    next_function_id: &mut usize,
    callable_defs: &mut Vec<BlockPyFunction<RuffBlockPyPass>>,
) -> Vec<Stmt> {
    let name_gen = NameGen::new(take_next_function_id(next_function_id));
    let mut lowered_plan = try_lower_function_to_blockpy_bundle(
        context,
        function_identity_by_node,
        func,
        current_parent,
        callable_semantic,
        &name_gen,
    );
    let param_names = lowered_plan.params.names();
    let param_name_set: HashSet<String> = param_names.iter().cloned().collect();
    let used_names: HashSet<String> = lowered_plan
        .blocks
        .iter()
        .flat_map(|block| loaded_names_in_blockpy_block(block).into_iter())
        .collect();
    let defined_names: HashSet<String> = lowered_plan
        .blocks
        .iter()
        .flat_map(|block| analyze_blockpy_use_def(block).1.into_iter())
        .collect();
    let mut local_cell_slots = lowered_plan.semantic.local_cell_storage_names();
    local_cell_slots.extend(lowered_plan.facts.cell_slots.iter().cloned());
    let function_id = lowered_plan.function_id.0;
    let captures = classify_capture_items(
        &used_names,
        &defined_names,
        &param_name_set,
        &local_cell_slots,
        &lowered_plan.semantic,
    );
    lowered_plan.facts.capture_storage_names = captures
        .iter()
        .map(|capture| capture.name.clone())
        .collect();
    lowered_plan.closure_layout = recompute_semantic_blockpy_closure_layout(&lowered_plan);
    let instantiation_kind = if lowered_plan.kind == BlockPyFunctionKind::Coroutine {
        LoweredFunctionInstantiationKind::MarkCoroutineFunction
    } else {
        LoweredFunctionInstantiationKind::DirectFunction
    };
    let identity =
        resolve_runtime_function_identity(func, function_identity_by_node, current_parent);
    debug_assert_eq!(
        parent_semantic.binding_target_for_name(identity.bind_name.as_str()),
        identity.binding_target,
        "function identity binding target disagrees with parent semantic binding for {}",
        identity.bind_name
    );
    let binding_target = resolve_function_binding_target(
        identity.binding_target,
        identity.bind_name.as_str(),
        identity.qualname.as_str(),
    );
    let bind_name = identity.bind_name.as_str();
    let annotate_helper = build_lowered_annotation_helper_binding(func, bind_name);
    let annotate_fn_expr = annotate_helper
        .as_ref()
        .map(|(_, annotate_fn_expr)| annotate_fn_expr.clone())
        .unwrap_or_else(|| py_expr!("None"));
    let (_, param_defaults) = collect_param_spec_and_defaults(&func.parameters);
    let decorated = build_lowered_function_instantiation_expr(
        function_id,
        &captures,
        rewrite_stmt::decorator::collect_exprs(&func.decorator_list),
        &param_defaults,
        annotate_fn_expr,
        instantiation_kind,
    );
    let mut binding_stmt =
        build_lowered_function_binding_stmt(bind_name, decorated, binding_target);
    if let Some((helper_stmt, _)) = annotate_helper {
        binding_stmt.insert(0, helper_stmt);
    }
    callable_defs.push(lowered_plan);
    if identity.bind_name.starts_with("_dp_class_ns_")
        || identity.bind_name.starts_with("_dp_define_class_")
    {
        let mut replacement = function_hoisted;
        replacement.extend(binding_stmt);
        replacement
    } else {
        parent_hoisted.extend(function_hoisted);
        binding_stmt
    }
}

impl BlockPyModuleRewriter<'_> {
    fn root_module_init_stmt<'a>(module: &'a mut Suite) -> &'a mut ast::StmtFunctionDef {
        module
            .iter_mut()
            .find_map(|stmt| match stmt {
                Stmt::FunctionDef(func) if func.name.id.as_str() == "_dp_module_init" => Some(func),
                _ => None,
            })
            .expect("missing _dp_module_init root function")
    }

    fn walk_function_def_with_scope(
        &mut self,
        func: &mut ast::StmtFunctionDef,
    ) -> FunctionScopeFrame {
        let fn_name = func.name.id.to_string();
        let function_scope = self.semantic_state.function_scope(func);
        let parent_name = self
            .function_scope_stack
            .last()
            .map(|frame| frame.name.clone());
        let callable_semantic =
            callable_semantic_info(function_scope.as_ref(), suite_ref(&func.body));
        self.function_scope_stack.push(FunctionScopeFrame {
            name: fn_name,
            parent_name,
            callable_semantic,
            hoisted_to_parent: Vec::new(),
        });
        self.visit_body(&mut func.body);
        self.function_scope_stack
            .pop()
            .expect("function scope stack should pop after walking function def")
    }

    fn lower_root_function_def(&mut self, func: &mut ast::StmtFunctionDef) {
        let state = self.walk_function_def_with_scope(func);
        assert!(
            state.hoisted_to_parent.is_empty(),
            "root _dp_module_init should not produce hoisted statements"
        );
        let name_gen = NameGen::new(take_next_function_id(&mut self.next_function_id));
        let lowered_plan = try_lower_function_to_blockpy_bundle(
            self.context,
            &self.function_identity_by_node,
            func,
            None,
            &state.callable_semantic,
            &name_gen,
        );
        self.callable_defs.push(lowered_plan);
    }

    fn rewrite_visited_function_def(
        &mut self,
        func: &mut ast::StmtFunctionDef,
        state: FunctionScopeFrame,
    ) -> Vec<Stmt> {
        let parent_frame = self
            .function_scope_stack
            .last_mut()
            .expect("nested function rewrite should always have a parent hoist buffer");
        let parent_semantic = parent_frame.callable_semantic.clone();
        let parent_hoisted = &mut parent_frame.hoisted_to_parent;
        rewrite_function_def_stmt_via_blockpy(
            self.context,
            parent_hoisted,
            &parent_semantic,
            &self.function_identity_by_node,
            func,
            state.parent_name.as_deref(),
            &state.callable_semantic,
            state.hoisted_to_parent,
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
            if let Stmt::FunctionDef(func) = &mut stmt {
                let state = self.walk_function_def_with_scope(func);
                let replacement = self.rewrite_visited_function_def(func, state);
                rewritten.extend(replacement);
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
