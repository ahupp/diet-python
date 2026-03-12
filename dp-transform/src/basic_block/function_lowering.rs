use super::annotation_export::{
    build_exec_function_def_binding_stmts, collect_capture_names, is_annotation_helper_name,
    rewrite_annotation_helper_defs_as_exec_calls, should_keep_non_lowered_for_annotationlib,
};
use super::await_lower::{coroutine_generator_marker_stmt, lower_coroutine_awaits_to_yield_from};
use super::bb_ir::BbExpr;
use super::block_py::state::{collect_cell_slots, collect_parameter_names};
use super::block_py::{
    BlockPyBlock, BlockPyBranchTable, BlockPyIf, BlockPyIfTerm, BlockPyRaise, BlockPyStmt,
    BlockPyTerm,
};
use super::bound_names::{collect_bound_names, collect_explicit_global_or_nonlocal_names};
use super::expr_utils::make_dp_tuple;
use super::function_identity::{
    is_module_init_temp_name, resolve_runtime_function_identity, FunctionIdentity,
};
use super::ruff_to_blockpy::{
    build_lowered_blockpy_function_bundle, lower_function_body_to_blockpy_function,
    LoweredBlockPyFunctionBundle,
};
use super::stmt_utils::{
    flatten_stmt_boxes, should_strip_nonlocal_for_bb, stmt_body_from_stmts,
    strip_nonlocal_directives,
};
use crate::basic_block::ast_to_ast::ast_rewrite::{rewrite_with_pass, Rewrite, StmtRewritePass};
use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::ast_to_ast::rewrite_import;
use crate::basic_block::ast_to_ast::rewrite_stmt;
use crate::basic_block::ast_to_ast::scope::{is_internal_symbol, Scope, ScopeKind};
use crate::driver::SimplifyExprPass;
use crate::py_expr;
use crate::transformer::{walk_expr, walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Expr, NodeIndex, Stmt, StmtBody};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

pub struct BBSimplifyStmtPass;

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
            for stmt in &if_stmt.body.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
            for clause in &if_stmt.elif_else_clauses {
                for stmt in &clause.body.body {
                    collect_deleted_names_in_stmt(stmt.as_ref(), names);
                }
            }
        }
        Stmt::While(while_stmt) => {
            for stmt in &while_stmt.body.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
            for stmt in &while_stmt.orelse.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
        }
        Stmt::For(for_stmt) => {
            for stmt in &for_stmt.body.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
            for stmt in &for_stmt.orelse.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
        }
        Stmt::Try(try_stmt) => {
            for stmt in &try_stmt.body.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
            for handler in &try_stmt.handlers {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                for stmt in &handler.body.body {
                    collect_deleted_names_in_stmt(stmt.as_ref(), names);
                }
            }
            for stmt in &try_stmt.orelse.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
            for stmt in &try_stmt.finalbody.body {
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

fn rewrite_deleted_name_loads(
    blocks: &mut [BlockPyBlock],
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
    stmt: &mut BlockPyStmt,
    rewriter: &mut DeletedNameLoadRewriter<'_>,
) {
    match stmt {
        BlockPyStmt::Pass
        | BlockPyStmt::Delete(_)
        | BlockPyStmt::Jump(_)
        | BlockPyStmt::TryJump(_) => {}
        BlockPyStmt::Expr(expr) => expr.rewrite_mut(|inner| rewriter.visit_expr(inner)),
        BlockPyStmt::Assign(assign) => assign.value.rewrite_mut(|expr| rewriter.visit_expr(expr)),
        BlockPyStmt::If(BlockPyIf { test, body, orelse }) => {
            test.rewrite_mut(|expr| rewriter.visit_expr(expr));
            for stmt in body {
                rewrite_blockpy_stmt_deleted_name_loads(stmt, rewriter);
            }
            for stmt in orelse {
                rewrite_blockpy_stmt_deleted_name_loads(stmt, rewriter);
            }
        }
        BlockPyStmt::BranchTable(BlockPyBranchTable { index, .. }) => {
            index.rewrite_mut(|expr| rewriter.visit_expr(expr))
        }
        BlockPyStmt::Return(Some(value)) => value.rewrite_mut(|expr| rewriter.visit_expr(expr)),
        BlockPyStmt::Return(None) => {}
        BlockPyStmt::Raise(BlockPyRaise { exc }) => {
            if let Some(exc) = exc {
                exc.rewrite_mut(|expr| rewriter.visit_expr(expr));
            }
        }
    }
}

fn rewrite_blockpy_term_deleted_name_loads(
    term: &mut BlockPyTerm,
    rewriter: &mut DeletedNameLoadRewriter<'_>,
) {
    match term {
        BlockPyTerm::Jump(_) | BlockPyTerm::TryJump(_) => {}
        BlockPyTerm::IfTerm(BlockPyIfTerm { test, .. }) => {
            test.rewrite_mut(|expr| rewriter.visit_expr(expr));
        }
        BlockPyTerm::BranchTable(BlockPyBranchTable { index, .. }) => {
            index.rewrite_mut(|expr| rewriter.visit_expr(expr))
        }
        BlockPyTerm::Return(Some(value)) => value.rewrite_mut(|expr| rewriter.visit_expr(expr)),
        BlockPyTerm::Return(None) => {}
        BlockPyTerm::Raise(BlockPyRaise { exc }) => {
            if let Some(exc) = exc {
                exc.rewrite_mut(|expr| rewriter.visit_expr(expr));
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
                self.visit_body(&mut if_stmt.body);
                for clause in if_stmt.elif_else_clauses.iter_mut() {
                    if let Some(test) = clause.test.as_mut() {
                        self.visit_expr(test);
                    }
                    self.visit_body(&mut clause.body);
                }
            }
            Stmt::While(while_stmt) => {
                self.visit_expr(while_stmt.test.as_mut());
                self.visit_body(&mut while_stmt.body);
                self.visit_body(&mut while_stmt.orelse);
            }
            Stmt::For(for_stmt) => {
                self.visit_expr(for_stmt.iter.as_mut());
                self.visit_body(&mut for_stmt.body);
                self.visit_body(&mut for_stmt.orelse);
            }
            Stmt::Try(try_stmt) => {
                self.visit_body(&mut try_stmt.body);
                for handler in try_stmt.handlers.iter_mut() {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    if let Some(type_) = handler.type_.as_mut() {
                        self.visit_expr(type_.as_mut());
                    }
                    self.visit_body(&mut handler.body);
                }
                self.visit_body(&mut try_stmt.orelse);
                self.visit_body(&mut try_stmt.finalbody);
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
    used_label_prefixes: &mut HashMap<String, usize>,
    next_block_id: &mut usize,
) -> Option<LoweredBlockPyFunctionBundle> {
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
            context,
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

    let end_label = next_label(func.name.id.as_str(), next_block_id);
    let identity = resolve_runtime_function_identity(func, function_identity_by_node, parent_name);
    let label_prefix = next_label_prefix(func.name.id.as_str(), used_label_prefixes);
    let mut local_next_block_id = *next_block_id;
    let mut prepared_function = lower_function_body_to_blockpy_function(
        func.name.id.as_str(),
        &runtime_input_body,
        identity.bind_name.clone(),
        identity.qualname.clone(),
        identity.binding_target,
        (*func.parameters).clone(),
        end_label,
        label_prefix.as_str(),
        has_yield,
        coroutine_via_generator,
        is_async_generator_runtime,
        is_closure_backed_generator_runtime,
        &cell_slots,
        &mut local_next_block_id,
        &mut |func_def| {
            build_exec_function_def_binding_stmts(func_def, &cell_slots, &outer_scope_names)
        },
        &mut |prefix, next_block_id| {
            next_temp_from_counter(reserved_temp_names_stack, prefix, next_block_id)
        },
    );
    *next_block_id = local_next_block_id;

    let mut blocks_for_dataflow = std::mem::take(&mut prepared_function.function.blocks);

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
    let enclosing_scope = module_scope
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
    let mut capture_names = collect_capture_names(func, enclosing_function_scope_names.as_ref());
    capture_names.sort();
    capture_names.dedup();
    let mut extra_closure_state_names = Vec::new();
    if is_closure_backed_generator_runtime {
        let mut bound_names = collect_bound_names(&runtime_input_body)
            .into_iter()
            .collect::<Vec<_>>();
        bound_names.sort();
        extra_closure_state_names.extend(bound_names);
        extra_closure_state_names.extend(capture_names.iter().cloned());
        extra_closure_state_names.sort();
        extra_closure_state_names.dedup();
    }
    prepared_function.function.blocks = blocks_for_dataflow;
    Some(build_lowered_blockpy_function_bundle(
        prepared_function,
        identity.display_name.clone(),
        has_yield,
        coroutine_via_generator,
        is_async_generator_runtime,
        is_closure_backed_generator_runtime,
        &param_names,
        &extra_closure_state_names,
        &capture_names,
        label_prefix.as_str(),
        cell_slots.clone(),
        is_module_init_temp_name(func.name.id.as_str()),
        BbExpr::from_expr(make_param_specs_expr(func.parameters.as_ref())),
    ))
}

pub(crate) fn function_docstring_expr(func: &ast::StmtFunctionDef) -> Option<Expr> {
    let (docstring, _) = split_docstring(&func.body);
    let Some(Stmt::Expr(expr_stmt)) = docstring else {
        return None;
    };
    Some(*expr_stmt.value)
}

pub(crate) fn lower_stmt_default(context: &Context, stmt: Stmt) -> Rewrite {
    match stmt {
        Stmt::Try(try_stmt) => rewrite_stmt::exception::rewrite_try(try_stmt),
        Stmt::If(if_stmt) => rewrite_stmt::loop_cond::expand_if_chain(if_stmt),
        Stmt::Assert(assert) => rewrite_stmt::assert::rewrite(assert),
        Stmt::Match(match_stmt) => rewrite_stmt::match_case::rewrite(context, match_stmt),
        Stmt::Import(import) => rewrite_import::rewrite(import),
        Stmt::ImportFrom(import_from) => rewrite_import::rewrite_from(context, import_from),
        Stmt::Assign(assign) => rewrite_stmt::assign_del::rewrite_assign(context, assign),
        Stmt::AugAssign(aug) => rewrite_stmt::assign_del::rewrite_aug_assign(context, aug),
        Stmt::Delete(del) => rewrite_stmt::assign_del::rewrite_delete(del),
        Stmt::Raise(raise) => rewrite_stmt::exception::rewrite_raise(raise),
        Stmt::TypeAlias(type_alias) => {
            rewrite_stmt::type_alias::rewrite_type_alias(context, type_alias)
        }
        Stmt::AnnAssign(_) => {
            panic!("should be removed by rewrite_ann_assign_to_dunder_annotate")
        }
        other => Rewrite::Unmodified(other),
    }
}

pub(crate) fn lower_stmt_bb(context: &Context, stmt: Stmt) -> Rewrite {
    match stmt {
        Stmt::Try(try_stmt) => lower_stmt_default(context, Stmt::Try(try_stmt)),
        other => lower_stmt_default(context, other),
    }
}

impl StmtRewritePass for BBSimplifyStmtPass {
    fn lower_stmt(&self, context: &Context, stmt: Stmt) -> Rewrite {
        lower_stmt_bb(context, stmt)
    }
}

struct BasicBlockSupportChecker {
    supported: bool,
    loop_depth: usize,
    allow_await: bool,
}

impl Default for BasicBlockSupportChecker {
    fn default() -> Self {
        Self {
            supported: true,
            loop_depth: 0,
            allow_await: false,
        }
    }
}

impl BasicBlockSupportChecker {
    fn mark_unsupported(&mut self) {
        self.supported = false;
    }

    fn panic_stmt(&self, message: &str, stmt: &Stmt) -> ! {
        let rendered = crate::ruff_ast_to_string(stmt);
        panic!(
            "BB lowering invariant violated: {message}\nstmt:\n{}",
            rendered.trim_end()
        );
    }
}

impl Transformer for BasicBlockSupportChecker {
    fn visit_body(&mut self, body: &mut StmtBody) {
        if !self.supported {
            return;
        }
        if has_dead_stmt_after_terminator(body) {
            self.mark_unsupported();
            return;
        }
        walk_stmt_body(self, body);
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if !self.supported {
            return;
        }
        match stmt {
            Stmt::Expr(_)
            | Stmt::Pass(_)
            | Stmt::Assign(_)
            | Stmt::Delete(_)
            | Stmt::Return(_)
            | Stmt::Raise(_) => {
                walk_stmt(self, stmt);
            }
            Stmt::FunctionDef(_) => {}
            Stmt::BodyStmt(_) => walk_stmt(self, stmt),
            Stmt::If(if_stmt) => {
                if if_stmt
                    .elif_else_clauses
                    .iter()
                    .any(|clause| clause.test.is_some())
                {
                    self.panic_stmt("`elif` chain reached support checker", stmt);
                }
                walk_stmt(self, stmt);
            }
            Stmt::While(while_stmt) => {
                self.visit_expr(while_stmt.test.as_mut());
                self.loop_depth += 1;
                self.visit_body(&mut while_stmt.body);
                self.loop_depth -= 1;
                self.visit_body(&mut while_stmt.orelse);
            }
            Stmt::For(for_stmt) => {
                if for_stmt.is_async && !self.allow_await {
                    self.mark_unsupported();
                    return;
                }
                self.visit_expr(for_stmt.iter.as_mut());
                self.loop_depth += 1;
                self.visit_body(&mut for_stmt.body);
                self.loop_depth -= 1;
                self.visit_body(&mut for_stmt.orelse);
            }
            Stmt::With(with_stmt) => {
                if with_stmt.is_async && !self.allow_await {
                    self.mark_unsupported();
                    return;
                }
                for item in with_stmt.items.iter_mut() {
                    self.visit_expr(&mut item.context_expr);
                    if let Some(optional_vars) = item.optional_vars.as_mut() {
                        self.visit_expr(optional_vars.as_mut());
                    }
                }
                self.visit_body(&mut with_stmt.body);
            }
            Stmt::Try(try_stmt) => {
                self.visit_body(&mut try_stmt.body);
                for handler in try_stmt.handlers.iter_mut() {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    if let Some(type_) = handler.type_.as_mut() {
                        self.visit_expr(type_.as_mut());
                    }
                    self.visit_body(&mut handler.body);
                }
                self.visit_body(&mut try_stmt.orelse);
                self.visit_body(&mut try_stmt.finalbody);
            }
            Stmt::Break(_) | Stmt::Continue(_) => {
                if self.loop_depth == 0 {
                    self.panic_stmt(
                        "`break`/`continue` outside loop reached support checker",
                        stmt,
                    );
                }
            }
            _ => self.panic_stmt("unsupported statement kind reached support checker", stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if !self.supported {
            return;
        }
        match expr {
            Expr::Await(_) => {
                if !self.allow_await {
                    self.mark_unsupported();
                    return;
                }
            }
            Expr::Yield(_) | Expr::YieldFrom(_) => {
                self.mark_unsupported();
                return;
            }
            _ => {}
        }
        walk_expr(self, expr);
    }
}

#[derive(Default)]
struct YieldLikeProbe {
    has_yield: bool,
    has_yield_from: bool,
    has_await: bool,
}

impl Transformer for YieldLikeProbe {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if matches!(stmt, Stmt::FunctionDef(_) | Stmt::ClassDef(_)) {
            return;
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Yield(_) => self.has_yield = true,
            Expr::YieldFrom(_) => self.has_yield_from = true,
            Expr::Await(_) => self.has_await = true,
            _ => {}
        }
        walk_expr(self, expr);
    }
}

pub(crate) fn has_yield_exprs_in_stmts(stmts: &[Box<Stmt>]) -> bool {
    let mut probe = YieldLikeProbe::default();
    for stmt in stmts {
        let mut stmt = stmt.as_ref().clone();
        probe.visit_stmt(&mut stmt);
        if probe.has_yield || probe.has_yield_from {
            return true;
        }
    }
    false
}

pub(crate) fn has_await_in_stmts(stmts: &[Box<Stmt>]) -> bool {
    let mut probe = YieldLikeProbe::default();
    for stmt in stmts {
        let mut stmt = stmt.as_ref().clone();
        probe.visit_stmt(&mut stmt);
        if probe.has_await {
            return true;
        }
    }
    false
}

fn make_param_specs_expr(parameters: &ast::Parameters) -> Expr {
    let mut specs = Vec::new();
    for param in &parameters.posonlyargs {
        push_param_specs(
            &mut specs,
            param.parameter.name.id.as_str(),
            "/",
            param.parameter.annotation.as_deref(),
            param.default.as_deref(),
        );
    }
    for param in &parameters.args {
        push_param_specs(
            &mut specs,
            param.parameter.name.id.as_str(),
            "",
            param.parameter.annotation.as_deref(),
            param.default.as_deref(),
        );
    }
    if let Some(param) = &parameters.vararg {
        push_param_specs(
            &mut specs,
            param.name.id.as_str(),
            "*",
            param.annotation.as_deref(),
            None,
        );
    }
    for param in &parameters.kwonlyargs {
        push_param_specs(
            &mut specs,
            param.parameter.name.id.as_str(),
            "kw:",
            param.parameter.annotation.as_deref(),
            param.default.as_deref(),
        );
    }
    if let Some(param) = &parameters.kwarg {
        push_param_specs(
            &mut specs,
            param.name.id.as_str(),
            "**",
            param.annotation.as_deref(),
            None,
        );
    }
    make_dp_tuple(specs)
}

fn push_param_specs(
    specs: &mut Vec<Expr>,
    name: &str,
    prefix: &str,
    _annotation: Option<&Expr>,
    default: Option<&Expr>,
) {
    let label = format!("{prefix}{name}");
    let annotation_expr = py_expr!("None");
    let default_expr = default
        .cloned()
        .unwrap_or_else(|| py_expr!("__dp__.NO_DEFAULT"));
    specs.push(make_dp_tuple(vec![
        py_expr!("{value:literal}", value = label.as_str()),
        annotation_expr,
        default_expr,
    ]));
}

fn split_docstring(body: &StmtBody) -> (Option<Stmt>, Vec<Box<Stmt>>) {
    let mut rest = body.body.clone();
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

fn walk_stmt_body<V: Transformer + ?Sized>(visitor: &mut V, body: &mut StmtBody) {
    for stmt in body.body.iter_mut() {
        visitor.visit_stmt(stmt.as_mut());
    }
}

fn has_dead_stmt_after_terminator(body: &StmtBody) -> bool {
    let mut terminated = false;
    for stmt in &body.body {
        if terminated {
            return true;
        }
        terminated = matches!(
            stmt.as_ref(),
            Stmt::Return(_) | Stmt::Raise(_) | Stmt::Break(_) | Stmt::Continue(_)
        );
    }
    false
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
        Stmt::BodyStmt(body) => has_dead_stmt_suffixes(&body.body),
        Stmt::If(if_stmt) => {
            has_dead_stmt_suffixes(&if_stmt.body.body)
                || if_stmt
                    .elif_else_clauses
                    .iter()
                    .any(|clause| has_dead_stmt_suffixes(&clause.body.body))
        }
        Stmt::While(while_stmt) => {
            has_dead_stmt_suffixes(&while_stmt.body.body)
                || has_dead_stmt_suffixes(&while_stmt.orelse.body)
        }
        Stmt::For(for_stmt) => {
            has_dead_stmt_suffixes(&for_stmt.body.body)
                || has_dead_stmt_suffixes(&for_stmt.orelse.body)
        }
        Stmt::Try(try_stmt) => {
            has_dead_stmt_suffixes(&try_stmt.body.body)
                || try_stmt.handlers.iter().any(|handler| {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    has_dead_stmt_suffixes(&handler.body.body)
                })
                || has_dead_stmt_suffixes(&try_stmt.orelse.body)
                || has_dead_stmt_suffixes(&try_stmt.finalbody.body)
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
        Stmt::BodyStmt(body) => {
            body.body = prune_dead_stmt_suffixes(&body.body);
        }
        Stmt::If(if_stmt) => {
            if_stmt.body.body = prune_dead_stmt_suffixes(&if_stmt.body.body);
            for clause in &mut if_stmt.elif_else_clauses {
                clause.body.body = prune_dead_stmt_suffixes(&clause.body.body);
            }
        }
        Stmt::While(while_stmt) => {
            while_stmt.body.body = prune_dead_stmt_suffixes(&while_stmt.body.body);
            while_stmt.orelse.body = prune_dead_stmt_suffixes(&while_stmt.orelse.body);
        }
        Stmt::For(for_stmt) => {
            for_stmt.body.body = prune_dead_stmt_suffixes(&for_stmt.body.body);
            for_stmt.orelse.body = prune_dead_stmt_suffixes(&for_stmt.orelse.body);
        }
        Stmt::Try(try_stmt) => {
            try_stmt.body.body = prune_dead_stmt_suffixes(&try_stmt.body.body);
            for handler in &mut try_stmt.handlers {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                handler.body.body = prune_dead_stmt_suffixes(&handler.body.body);
            }
            try_stmt.orelse.body = prune_dead_stmt_suffixes(&try_stmt.orelse.body);
            try_stmt.finalbody.body = prune_dead_stmt_suffixes(&try_stmt.finalbody.body);
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

fn next_label_prefix(fn_name: &str, used_label_prefixes: &mut HashMap<String, usize>) -> String {
    let base = sanitize_ident(original_function_name(fn_name).as_str());
    let count = used_label_prefixes.entry(base.clone()).or_insert(0);
    let suffix = if *count == 0 {
        String::new()
    } else {
        format!("_{}", *count)
    };
    *count += 1;
    format!("_dp_bb_{base}{suffix}")
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

fn original_function_name(fn_name: &str) -> String {
    let Some(rest) = fn_name.strip_prefix("_dp_fn_") else {
        return fn_name.to_string();
    };
    let Some((prefix, trailing)) = rest.rsplit_once('_') else {
        return rest.to_string();
    };
    if !trailing.is_empty() && trailing.chars().all(|ch| ch.is_ascii_digit()) {
        prefix.to_string()
    } else {
        rest.to_string()
    }
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
