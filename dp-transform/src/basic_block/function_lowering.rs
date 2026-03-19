use super::annotation_export::{
    is_annotation_helper_name, rewrite_annotation_helper_defs_as_exec_calls,
    should_keep_non_lowered_for_annotationlib,
};
use super::block_py::state::collect_cell_slots;
use super::block_py::{
    BlockPyBlock, BlockPyBranchTable, BlockPyCallableDef, BlockPyCallableFacts,
    BlockPyFunctionKind, BlockPyIf, BlockPyIfTerm, BlockPyRaise, BlockPyStmt, BlockPyStmtFragment,
    BlockPyTerm, FunctionName,
};
use super::bound_names::{collect_bound_names, collect_explicit_global_or_nonlocal_names};
use super::function_identity::{resolve_runtime_function_identity, FunctionIdentity};
use super::ruff_to_blockpy::{
    build_blockpy_callable_def_from_runtime_input, take_next_function_id,
};
use super::stmt_utils::{
    flatten_stmt_boxes, should_strip_nonlocal_for_bb, strip_nonlocal_directives,
};
use crate::basic_block::ast_to_ast::ast_rewrite::{Rewrite, StmtRewritePass};
use crate::basic_block::ast_to_ast::body::{suite_mut, suite_ref, Suite};
use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::ast_to_ast::scope::{is_internal_symbol, Scope};
use crate::basic_block::block_py::param_specs::collect_param_spec_and_defaults;
use crate::py_expr;
use crate::transformer::{walk_expr, walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Expr, NodeIndex, Stmt};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct SingleNamedAssignmentPass;

fn collect_deleted_names(stmts: &[Box<Stmt>]) -> HashSet<String> {
    let mut names = HashSet::new();
    for stmt in stmts {
        collect_deleted_names_in_stmt(stmt.as_ref(), &mut names);
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
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
            for clause in &if_stmt.elif_else_clauses {
                for stmt in suite_ref(&clause.body) {
                    collect_deleted_names_in_stmt(stmt.as_ref(), names);
                }
            }
        }
        Stmt::While(while_stmt) => {
            for stmt in suite_ref(&while_stmt.body) {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
            for stmt in suite_ref(&while_stmt.orelse) {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
        }
        Stmt::For(for_stmt) => {
            for stmt in suite_ref(&for_stmt.body) {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
            for stmt in suite_ref(&for_stmt.orelse) {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
        }
        Stmt::Try(try_stmt) => {
            for stmt in suite_ref(&try_stmt.body) {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
            for handler in &try_stmt.handlers {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                for stmt in suite_ref(&handler.body) {
                    collect_deleted_names_in_stmt(stmt.as_ref(), names);
                }
            }
            for stmt in suite_ref(&try_stmt.orelse) {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
            for stmt in suite_ref(&try_stmt.finalbody) {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
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

fn rewrite_blockpy_expr_deleted_name_loads(
    expr: &mut Expr,
    rewriter: &mut DeletedNameLoadRewriter<'_>,
) {
    rewriter.visit_expr(expr);
}

pub(crate) fn rewrite_deleted_name_loads(
    blocks: &mut [BlockPyBlock<Expr>],
    deleted_names: &HashSet<String>,
    always_unbound_names: &HashSet<String>,
) {
    let mut rewriter = DeletedNameLoadRewriter {
        deleted_names,
        always_unbound_names,
    };
    for block in blocks {
        for stmt in block.body.iter_mut() {
            rewrite_blockpy_stmt_deleted_name_loads(stmt, &mut rewriter);
        }
        rewrite_blockpy_term_deleted_name_loads(&mut block.term, &mut rewriter);
    }
}

fn rewrite_blockpy_stmt_deleted_name_loads(
    stmt: &mut BlockPyStmt<Expr>,
    rewriter: &mut DeletedNameLoadRewriter<'_>,
) {
    match stmt {
        BlockPyStmt::Delete(_) => {}
        BlockPyStmt::Expr(expr) => rewrite_blockpy_expr_deleted_name_loads(expr, rewriter),
        BlockPyStmt::Assign(assign) => {
            rewrite_blockpy_expr_deleted_name_loads(&mut assign.value, rewriter)
        }
        BlockPyStmt::If(BlockPyIf { test, body, orelse }) => {
            rewrite_blockpy_expr_deleted_name_loads(test, rewriter);
            rewrite_blockpy_stmt_fragment_deleted_name_loads(body, rewriter);
            rewrite_blockpy_stmt_fragment_deleted_name_loads(orelse, rewriter);
        }
    }
}

fn rewrite_blockpy_stmt_fragment_deleted_name_loads(
    fragment: &mut BlockPyStmtFragment<Expr>,
    rewriter: &mut DeletedNameLoadRewriter<'_>,
) {
    for stmt in &mut fragment.body {
        rewrite_blockpy_stmt_deleted_name_loads(stmt, rewriter);
    }
    if let Some(term) = &mut fragment.term {
        rewrite_blockpy_term_deleted_name_loads(term, rewriter);
    }
}

fn rewrite_blockpy_term_deleted_name_loads(
    term: &mut BlockPyTerm<Expr>,
    rewriter: &mut DeletedNameLoadRewriter<'_>,
) {
    match term {
        BlockPyTerm::Jump(_) | BlockPyTerm::TryJump(_) => {}
        BlockPyTerm::IfTerm(BlockPyIfTerm { test, .. }) => {
            rewrite_blockpy_expr_deleted_name_loads(test, rewriter);
        }
        BlockPyTerm::BranchTable(BlockPyBranchTable { index, .. }) => {
            rewrite_blockpy_expr_deleted_name_loads(index, rewriter)
        }
        BlockPyTerm::Return(Some(value)) => {
            rewrite_blockpy_expr_deleted_name_loads(value, rewriter)
        }
        BlockPyTerm::Return(None) => {}
        BlockPyTerm::Raise(BlockPyRaise { exc }) => {
            if let Some(exc) = exc {
                rewrite_blockpy_expr_deleted_name_loads(exc, rewriter);
            }
        }
    }
}

struct DeletedNameLoadRewriter<'a> {
    deleted_names: &'a HashSet<String>,
    always_unbound_names: &'a HashSet<String>,
}

impl Transformer for DeletedNameLoadRewriter<'_> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) | Stmt::Delete(_) => {}
            Stmt::Expr(expr_stmt) => self.visit_expr(expr_stmt.value.as_mut()),
            Stmt::Assign(assign) => {
                self.visit_expr(assign.value.as_mut());
            }
            Stmt::AugAssign(aug_assign) => {
                self.visit_expr(aug_assign.target.as_mut());
                self.visit_expr(aug_assign.value.as_mut());
            }
            Stmt::Return(ret) => {
                if let Some(value) = ret.value.as_mut() {
                    self.visit_expr(value.as_mut());
                }
            }
            Stmt::If(if_stmt) => {
                self.visit_expr(if_stmt.test.as_mut());
                self.visit_body(suite_mut(&mut if_stmt.body));
                for clause in if_stmt.elif_else_clauses.iter_mut() {
                    if let Some(test) = clause.test.as_mut() {
                        self.visit_expr(test);
                    }
                    self.visit_body(suite_mut(&mut clause.body));
                }
            }
            Stmt::While(while_stmt) => {
                self.visit_expr(while_stmt.test.as_mut());
                self.visit_body(suite_mut(&mut while_stmt.body));
                self.visit_body(suite_mut(&mut while_stmt.orelse));
            }
            Stmt::For(for_stmt) => {
                self.visit_expr(for_stmt.iter.as_mut());
                self.visit_body(suite_mut(&mut for_stmt.body));
                self.visit_body(suite_mut(&mut for_stmt.orelse));
            }
            Stmt::Try(try_stmt) => {
                self.visit_body(suite_mut(&mut try_stmt.body));
                for handler in try_stmt.handlers.iter_mut() {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    if let Some(type_) = handler.type_.as_mut() {
                        self.visit_expr(type_.as_mut());
                    }
                    self.visit_body(suite_mut(&mut handler.body));
                }
                self.visit_body(suite_mut(&mut try_stmt.orelse));
                self.visit_body(suite_mut(&mut try_stmt.finalbody));
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Name(name) = expr {
            if matches!(name.ctx, ast::ExprContext::Load) {
                let always_unbound = self.always_unbound_names.contains(name.id.as_str());
                let deleted = self.deleted_names.contains(name.id.as_str());
                if !always_unbound && !deleted {
                    walk_expr(self, expr);
                    return;
                }
                let value = if always_unbound {
                    py_expr!("__dp_DELETED")
                } else {
                    Expr::Name(name.clone())
                };
                let name_value = name.id.to_string();
                *expr = py_expr!(
                    "__dp_load_deleted_name({name:literal}, {value:expr})",
                    name = name_value.as_str(),
                    value = value,
                );
                return;
            }
        }
        walk_expr(self, expr);
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

pub(crate) fn try_lower_function_to_blockpy_bundle(
    context: &Context,
    module_scope: &Arc<Scope>,
    function_identity_by_node: &HashMap<NodeIndex, FunctionIdentity>,
    func: &ast::StmtFunctionDef,
    parent_name: Option<&str>,
    reserved_temp_names_stack: &mut Vec<HashSet<String>>,
    next_block_id: &mut usize,
    next_function_id: &mut usize,
) -> Option<BlockPyCallableDef<Expr>> {
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
    let lowered_input_body = flatten_stmt_boxes(&lowered_input_body);
    let lowered_input_body = if should_strip_nonlocal_for_bb(func.name.id.as_str()) {
        strip_nonlocal_directives(lowered_input_body)
    } else {
        lowered_input_body
    };
    let (param_spec, param_defaults) = collect_param_spec_and_defaults(&func.parameters);
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
    let blockpy_kind = if func.is_async {
        BlockPyFunctionKind::Coroutine
    } else {
        BlockPyFunctionKind::Function
    };
    let callable_def = build_blockpy_callable_def_from_runtime_input(
        context,
        main_function_id,
        FunctionName::new(
            identity.bind_name.clone(),
            fn_name,
            identity.display_name.clone(),
            identity.qualname.clone(),
        ),
        param_spec,
        param_defaults,
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
    Some(callable_def)
}

pub(crate) fn function_docstring_text(func: &ast::StmtFunctionDef) -> Option<String> {
    let (docstring, _) = split_docstring(suite_ref(&func.body));
    let Some(Stmt::Expr(expr_stmt)) = docstring else {
        return None;
    };
    let Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) = *expr_stmt.value else {
        return None;
    };
    Some(value.to_string())
}

pub(crate) fn lower_stmt_default(context: &Context, stmt: Stmt) -> Rewrite {
    match stmt {
        Stmt::Assign(assign) => {
            crate::basic_block::ruff_to_blockpy::rewrite_assign_stmt(context, assign)
        }
        Stmt::Delete(del) => crate::basic_block::ruff_to_blockpy::rewrite_delete_stmt(del),
        other => Rewrite::Unmodified(other),
    }
}

pub(crate) fn lower_stmt_bb(context: &Context, stmt: Stmt) -> Rewrite {
    lower_stmt_default(context, stmt)
}

impl StmtRewritePass for SingleNamedAssignmentPass {
    fn lower_stmt(&self, context: &Context, stmt: Stmt) -> Rewrite {
        lower_stmt_bb(context, stmt)
    }
}

fn split_docstring(body: &Suite) -> (Option<Stmt>, Vec<Box<Stmt>>) {
    let mut rest = body.clone();
    let Some(first) = rest.first() else {
        return (None, rest);
    };
    if matches!(
        first.as_ref(),
        Stmt::Expr(ast::StmtExpr { value, .. }) if matches!(value.as_ref(), Expr::StringLiteral(_))
    ) {
        let first_stmt = *rest.remove(0);
        return (Some(first_stmt), rest);
    }
    (None, rest)
}

fn has_dead_stmt_suffixes(stmts: &[Box<Stmt>]) -> bool {
    let mut terminated = false;
    for stmt in stmts {
        let stmt = stmt.as_ref();
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

fn prune_dead_stmt_suffixes(stmts: &[Box<Stmt>]) -> Vec<Box<Stmt>> {
    let mut out = Vec::new();
    for stmt in stmts {
        let mut stmt = stmt.as_ref().clone();
        prune_dead_stmt_suffixes_in_stmt(&mut stmt);
        let terminates = matches!(
            stmt,
            Stmt::Return(_) | Stmt::Raise(_) | Stmt::Break(_) | Stmt::Continue(_)
        );
        out.push(Box::new(stmt));
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
    lowered_input_body: &[Box<Stmt>],
    runtime_body: &[Box<Stmt>],
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
