use crate::block_py::param_specs::{collect_param_spec_and_defaults, param_defaults_to_expr};
use crate::block_py::{
    derive_effective_binding_for_name, BindingTarget, BlockPyBindingKind, BlockPyBindingPurpose,
    BlockPyCellBindingKind,
};
use crate::block_py::{
    BlockPyCallableFacts, BlockPyCallableScopeKind, BlockPyCallableSemanticInfo, BlockPyFunction,
    BlockPyFunctionKind, BlockPyModule, ClosureLayout, FunctionName, FunctionNameGen,
    ModuleNameGen,
};
use crate::passes::ast_symbol_analysis::{
    collect_bound_names, collect_explicit_global_or_nonlocal_names, collect_loaded_names,
};
use crate::passes::ast_to_ast::body::{split_docstring, suite_mut, suite_ref, Suite};
use crate::passes::ast_to_ast::context::Context;
use crate::passes::ast_to_ast::expr_utils::{make_dp_tuple, name_expr};
use crate::passes::ast_to_ast::rewrite_stmt;
use crate::passes::ast_to_ast::scope_helpers::is_internal_symbol;
use crate::passes::ast_to_ast::semantic::{
    SemanticAstState, SemanticBindingKind, SemanticScope, SemanticScopeKind,
};
use crate::passes::ast_to_ast::util::{
    strip_synthetic_class_namespace_qualname, strip_synthetic_module_init_qualname,
};
use crate::passes::ruff_to_blockpy::recompute_semantic_blockpy_closure_layout;
use crate::passes::RuffBlockPyPass;
use crate::transformer::{walk_expr, walk_stmt, Transformer};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt};
use std::collections::{HashMap, HashSet};

use super::{build_blockpy_callable_def_from_runtime_input, rewrite_deleted_name_loads};

struct FunctionScopeFrame {
    scope: Option<SemanticScope>,
    callable_semantic: BlockPyCallableSemanticInfo,
    hoisted_to_parent: Vec<Stmt>,
}

struct BlockPyModuleRewriter<'a> {
    context: &'a Context,
    semantic_state: &'a SemanticAstState,
    module_name_gen: ModuleNameGen,
    function_scope_stack: Vec<FunctionScopeFrame>,
    callable_defs: Vec<BlockPyFunction<RuffBlockPyPass>>,
}

#[derive(Default)]
struct YieldFamilyDetector {
    found: bool,
}

pub(crate) fn rewrite_ast_to_lowered_blockpy_module_plan_with_module(
    context: &Context,
    module: &mut Suite,
    semantic_state: &SemanticAstState,
) -> BlockPyModule<RuffBlockPyPass> {
    crate::passes::ast_to_ast::simplify::flatten(module);
    let mut rewriter = BlockPyModuleRewriter {
        context,
        semantic_state,
        module_name_gen: ModuleNameGen::new(0),
        function_scope_stack: Vec::new(),
        callable_defs: Vec::new(),
    };
    let module_init = BlockPyModuleRewriter::root_module_init_stmt(module);
    rewriter.lower_root_function_def(module_init);
    BlockPyModule {
        callable_defs: rewriter.callable_defs,
    }
}

fn is_module_init_name(name: &str) -> bool {
    name == "_dp_module_init" || name.starts_with("_dp_fn__dp_module_init_")
}

fn display_name_for_function(raw_name: &str) -> &str {
    if raw_name.starts_with("_dp_lambda_") {
        "<lambda>"
    } else if raw_name.starts_with("_dp_genexpr_") {
        "<genexpr>"
    } else if raw_name.starts_with("_dp_listcomp_") {
        "<listcomp>"
    } else if raw_name.starts_with("_dp_setcomp_") {
        "<setcomp>"
    } else if raw_name.starts_with("_dp_dictcomp_") {
        "<dictcomp>"
    } else {
        raw_name
    }
}

fn normalize_qualname(raw_qualname: &str, raw_name: &str, display_name: &str) -> String {
    let raw_qualname = strip_synthetic_module_init_qualname(raw_qualname);
    let raw_qualname = strip_synthetic_class_namespace_qualname(&raw_qualname);
    let should_replace_tail = matches!(display_name, "<lambda>" | "<genexpr>");
    if raw_name == display_name || !should_replace_tail {
        return raw_qualname;
    }
    match raw_qualname.rsplit_once('.') {
        Some((prefix, _)) => format!("{prefix}.{display_name}"),
        None => display_name.to_string(),
    }
}

fn callable_semantic_info(
    semantic_state: &SemanticAstState,
    parent_scope: Option<&SemanticScope>,
    function_scope: Option<&SemanticScope>,
    func: Option<&ast::StmtFunctionDef>,
    body: &[Stmt],
) -> BlockPyCallableSemanticInfo {
    let Some(function_scope) = function_scope else {
        return BlockPyCallableSemanticInfo::default();
    };
    let scope_kind = match function_scope.kind() {
        SemanticScopeKind::Function => BlockPyCallableScopeKind::Function,
        SemanticScopeKind::Class => BlockPyCallableScopeKind::Class,
        SemanticScopeKind::Module => BlockPyCallableScopeKind::Module,
    };
    let local_cell_bindings = function_scope.local_cell_bindings();
    let type_param_names = function_scope.type_param_names();
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
                    scope_kind,
                    &type_param_names,
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
                scope_kind,
                &type_param_names,
            )
        });
    }
    let effective_load_bindings = bindings
        .iter()
        .map(|(name, binding)| {
            (
                name.clone(),
                derive_effective_binding_for_name(
                    name.as_str(),
                    *binding,
                    scope_kind,
                    &type_param_names,
                    BlockPyBindingPurpose::Load,
                    false,
                ),
            )
        })
        .collect();
    let effective_store_bindings = bindings
        .iter()
        .map(|(name, binding)| {
            (
                name.clone(),
                derive_effective_binding_for_name(
                    name.as_str(),
                    *binding,
                    scope_kind,
                    &type_param_names,
                    BlockPyBindingPurpose::Store,
                    false,
                ),
            )
        })
        .collect();
    let names = match func {
        Some(func) => {
            let raw_bind_name = func.name.id.to_string();
            let bind_name = if is_module_init_name(raw_bind_name.as_str()) {
                "_dp_module_init".to_string()
            } else {
                raw_bind_name.clone()
            };
            let display_name = display_name_for_function(bind_name.as_str()).to_string();
            let qualname = if is_module_init_name(raw_bind_name.as_str()) {
                "_dp_module_init".to_string()
            } else if semantic_state.has_function_scope_override(func) {
                normalize_qualname(
                    parent_scope
                        .expect("missing parent scope for function scope override")
                        .child_function_qualname(raw_bind_name.as_str())
                        .as_str(),
                    bind_name.as_str(),
                    display_name.as_str(),
                )
            } else {
                normalize_qualname(
                    function_scope.qualname(),
                    bind_name.as_str(),
                    display_name.as_str(),
                )
            };
            FunctionName::new(bind_name, raw_bind_name, display_name, qualname)
        }
        None => FunctionName::default(),
    };
    BlockPyCallableSemanticInfo {
        names,
        scope_kind,
        bindings,
        cell_storage_names: HashMap::new(),
        semantic_internal_names: HashSet::new(),
        type_param_names,
        effective_load_bindings,
        effective_store_bindings,
    }
}

fn blockpy_binding_kind_for_name(
    name: &str,
    binding: SemanticBindingKind,
    local_cell_bindings: &HashSet<String>,
    has_local_def: bool,
    scope_kind: BlockPyCallableScopeKind,
    type_param_names: &HashSet<String>,
) -> BlockPyBindingKind {
    if scope_kind == BlockPyCallableScopeKind::Class
        && has_local_def
        && !type_param_names.contains(name)
    {
        return BlockPyBindingKind::Local;
    }
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
    func: &ast::StmtFunctionDef,
    callable_semantic: &BlockPyCallableSemanticInfo,
    name_gen: FunctionNameGen,
) -> BlockPyFunction<RuffBlockPyPass> {
    let (docstring, lowered_input_body) = split_docstring(suite_ref(&func.body));
    let lowered_input_body = lowered_input_body.to_vec();
    let (param_spec, _param_defaults) = collect_param_spec_and_defaults(&func.parameters);
    let param_names = param_spec.names();
    let runtime_input_body = prune_dead_stmt_suffixes(&lowered_input_body);
    let unbound_local_names = if has_dead_stmt_suffixes(&lowered_input_body) {
        always_unbound_local_names(&lowered_input_body, &runtime_input_body, &param_names)
    } else {
        HashSet::new()
    };
    let deleted_names = collect_deleted_names(&runtime_input_body);
    let callable_facts = BlockPyCallableFacts {
        deleted_names,
        unbound_local_names,
    };

    let end_label = name_gen.next_block_name();
    let doc = docstring;
    let blockpy_kind = function_kind(func);
    let mut callable_def = build_blockpy_callable_def_from_runtime_input(
        context,
        name_gen,
        callable_semantic.names.clone(),
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
fn capture_items_to_expr(captures: &[(String, Expr)]) -> Expr {
    make_dp_tuple(
        captures
            .iter()
            .map(|(name, value_expr)| {
                make_dp_tuple(vec![
                    py_expr!("{value:literal}", value = name.as_str()),
                    value_expr.clone(),
                ])
            })
            .collect(),
    )
}

fn closure_freevar_capture_items(closure_layout: Option<&ClosureLayout>) -> Vec<(String, Expr)> {
    closure_layout
        .into_iter()
        .flat_map(|layout| layout.freevars.iter())
        .map(|slot| {
            (
                slot.storage_name.clone(),
                name_expr(slot.storage_name.as_str())
                    .expect("capture storage name should always parse as an expression"),
            )
        })
        .collect()
}

fn build_lowered_function_instantiation_expr(
    function_id: crate::block_py::FunctionId,
    closure_layout: Option<&ClosureLayout>,
    decorator_exprs: Vec<Expr>,
    param_defaults: &[Expr],
    annotate_fn_expr: Expr,
    kind: BlockPyFunctionKind,
) -> Expr {
    let captures = closure_freevar_capture_items(closure_layout);
    let capture_expr = capture_items_to_expr(&captures);
    let param_defaults_expr = param_defaults_to_expr(param_defaults);
    let kind_name = match kind {
        BlockPyFunctionKind::Function => "function",
        BlockPyFunctionKind::Coroutine => "coroutine",
        BlockPyFunctionKind::Generator => "generator",
        BlockPyFunctionKind::AsyncGenerator => "async_generator",
    };
    let base_function_expr = py_expr!(
        "__dp_make_function({function_id:literal}, {kind:literal}, {closure:expr}, {param_defaults:expr}, {module_globals:expr}, {annotate_fn:expr})",
        function_id = function_id.0,
        kind = kind_name,
        closure = capture_expr.clone(),
        param_defaults = param_defaults_expr.clone(),
        module_globals = py_expr!("__dp_globals()"),
        annotate_fn = annotate_fn_expr.clone(),
    );
    rewrite_stmt::decorator::rewrite_exprs(decorator_exprs, base_function_expr)
}

fn build_lowered_function_binding_stmt(
    bind_name: &str,
    value: Expr,
    target: BindingTarget,
) -> Vec<Stmt> {
    match target {
        BindingTarget::Local | BindingTarget::ModuleGlobal | BindingTarget::ClassNamespace => {
            vec![py_stmt!(
                "{name:id} = {value:expr}",
                name = bind_name,
                value = value
            )]
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn rewrite_function_def_stmt_via_blockpy(
    context: &Context,
    parent_hoisted: &mut Vec<Stmt>,
    parent_semantic: &BlockPyCallableSemanticInfo,
    func: &mut ast::StmtFunctionDef,
    callable_semantic: &BlockPyCallableSemanticInfo,
    function_hoisted: Vec<Stmt>,
    module_name_gen: &mut ModuleNameGen,
    callable_defs: &mut Vec<BlockPyFunction<RuffBlockPyPass>>,
) -> Vec<Stmt> {
    let name_gen = module_name_gen.next_function_name_gen();
    let mut lowered_plan =
        try_lower_function_to_blockpy_bundle(context, func, callable_semantic, name_gen);
    lowered_plan.closure_layout = recompute_semantic_blockpy_closure_layout(&lowered_plan);
    let bind_name = lowered_plan.names.bind_name.clone();
    let binding_target =
        parent_semantic.binding_target_for_name(bind_name.as_str(), BlockPyBindingPurpose::Store);
    let (_, param_defaults) = collect_param_spec_and_defaults(&func.parameters);
    let decorated = build_lowered_function_instantiation_expr(
        lowered_plan.function_id,
        lowered_plan.closure_layout.as_ref(),
        rewrite_stmt::decorator::collect_exprs(&func.decorator_list),
        &param_defaults,
        py_expr!("None"),
        lowered_plan.kind,
    );
    let binding_stmt =
        build_lowered_function_binding_stmt(bind_name.as_str(), decorated, binding_target);
    callable_defs.push(lowered_plan);
    if bind_name.starts_with("_dp_class_ns_") || bind_name.starts_with("_dp_define_class_") {
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
        assert_eq!(
            module.len(),
            1,
            "expected root suite with exactly one statement",
        );
        let Stmt::FunctionDef(func) = &mut module[0] else {
            panic!("expected root suite with exactly one function");
        };
        assert!(
            func.parameters.posonlyargs.is_empty()
                && func.parameters.args.is_empty()
                && func.parameters.vararg.is_none()
                && func.parameters.kwonlyargs.is_empty()
                && func.parameters.kwarg.is_none(),
            "expected root function with no parameters",
        );
        func
    }

    fn walk_function_def_with_scope(
        &mut self,
        func: &mut ast::StmtFunctionDef,
    ) -> FunctionScopeFrame {
        let function_scope = self.semantic_state.function_scope(func);
        let parent_scope = self
            .function_scope_stack
            .last()
            .and_then(|frame| frame.scope.as_ref())
            .cloned();
        let callable_semantic = callable_semantic_info(
            self.semantic_state,
            parent_scope.as_ref(),
            function_scope.as_ref(),
            Some(func),
            suite_ref(&func.body),
        );
        self.function_scope_stack.push(FunctionScopeFrame {
            scope: function_scope.clone(),
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
        let name_gen = self.module_name_gen.next_function_name_gen();
        let lowered_plan = try_lower_function_to_blockpy_bundle(
            self.context,
            func,
            &state.callable_semantic,
            name_gen,
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
            func,
            &state.callable_semantic,
            state.hoisted_to_parent,
            &mut self.module_name_gen,
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

#[cfg(test)]
mod test;
