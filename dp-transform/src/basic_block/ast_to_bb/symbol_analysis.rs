use super::Terminator;
use crate::transformer::{walk_expr, Transformer};
use ruff_python_ast::{self as ast, Expr, Stmt};
use std::collections::HashSet;

#[derive(Default)]
struct LoadNameCollector {
    names: HashSet<String>,
}

impl Transformer for LoadNameCollector {
    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Name(name) = expr {
            if matches!(name.ctx, ast::ExprContext::Load) {
                self.names.insert(name.id.to_string());
            }
        }
        walk_expr(self, expr);
    }
}

pub(super) fn load_names_in_expr(expr: &Expr) -> HashSet<String> {
    let mut expr = expr.clone();
    let mut collector = LoadNameCollector::default();
    collector.visit_expr(&mut expr);
    collector.names
}

pub(super) fn load_names_in_stmt(stmt: &Stmt) -> HashSet<String> {
    match stmt {
        Stmt::Expr(expr_stmt) => load_names_in_expr(expr_stmt.value.as_ref()),
        Stmt::Assign(assign) => load_names_in_expr(assign.value.as_ref()),
        Stmt::Raise(raise_stmt) => {
            let mut names = HashSet::new();
            if let Some(exc) = raise_stmt.exc.as_ref() {
                names.extend(load_names_in_expr(exc.as_ref()));
            }
            if let Some(cause) = raise_stmt.cause.as_ref() {
                names.extend(load_names_in_expr(cause.as_ref()));
            }
            names
        }
        Stmt::If(if_stmt) => {
            let mut names = load_names_in_expr(if_stmt.test.as_ref());
            for stmt in &if_stmt.body.body {
                names.extend(load_names_in_stmt(stmt.as_ref()));
            }
            for clause in &if_stmt.elif_else_clauses {
                if let Some(test) = clause.test.as_ref() {
                    names.extend(load_names_in_expr(test));
                }
                for stmt in &clause.body.body {
                    names.extend(load_names_in_stmt(stmt.as_ref()));
                }
            }
            names
        }
        Stmt::While(while_stmt) => {
            let mut names = load_names_in_expr(while_stmt.test.as_ref());
            for stmt in &while_stmt.body.body {
                names.extend(load_names_in_stmt(stmt.as_ref()));
            }
            for stmt in &while_stmt.orelse.body {
                names.extend(load_names_in_stmt(stmt.as_ref()));
            }
            names
        }
        Stmt::For(for_stmt) => {
            let mut names = load_names_in_expr(for_stmt.iter.as_ref());
            names.extend(load_names_in_expr(for_stmt.target.as_ref()));
            for stmt in &for_stmt.body.body {
                names.extend(load_names_in_stmt(stmt.as_ref()));
            }
            for stmt in &for_stmt.orelse.body {
                names.extend(load_names_in_stmt(stmt.as_ref()));
            }
            names
        }
        Stmt::Try(try_stmt) => {
            let mut names = HashSet::new();
            let mut defs = HashSet::new();
            for stmt in &try_stmt.body.body {
                names.extend(load_names_in_stmt(stmt.as_ref()));
                defs.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
            for handler in &try_stmt.handlers {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                if let Some(type_) = handler.type_.as_ref() {
                    names.extend(load_names_in_expr(type_.as_ref()));
                }
                for stmt in &handler.body.body {
                    names.extend(load_names_in_stmt(stmt.as_ref()));
                    defs.extend(assigned_names_in_stmt(stmt.as_ref()));
                }
            }
            for stmt in &try_stmt.orelse.body {
                names.extend(load_names_in_stmt(stmt.as_ref()));
                defs.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
            for stmt in &try_stmt.finalbody.body {
                names.extend(load_names_in_stmt(stmt.as_ref()));
                defs.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
            names.retain(|name| {
                !defs.contains(name) || name.starts_with("_dp_cell_") || name == "_dp_classcell"
            });
            names
        }
        Stmt::Delete(delete_stmt) => {
            let mut names = HashSet::new();
            for target in &delete_stmt.targets {
                names.extend(load_names_in_expr(target));
            }
            names
        }
        Stmt::FunctionDef(func_def) => {
            // A function definition evaluates only header-time expressions
            // (decorators/defaults/annotations/type params) when the `def`
            // statement runs.
            let mut header_only = func_def.clone();
            header_only.body.body.clear();
            let mut stmt = Stmt::FunctionDef(header_only);
            let mut collector = LoadNameCollector::default();
            collector.visit_stmt(&mut stmt);
            let mut names = collector.names;

            if !func_def.name.id.as_str().starts_with("_dp_bb_") {
                // Nested transformed non-BB helper functions can require outer
                // closure cells at definition time so the created function
                // captures those cells. BB helper defs thread cells explicitly
                // via parameters/closure tuples and should not force the outer
                // function's entry-params.
                let mut full_stmt = Stmt::FunctionDef(func_def.clone());
                let mut body_collector = LoadNameCollector::default();
                body_collector.visit_stmt(&mut full_stmt);
                for name in body_collector.names {
                    if name.starts_with("_dp_cell_") {
                        names.insert(name);
                    }
                }
            }

            names
        }
        Stmt::Return(ret) => ret
            .value
            .as_ref()
            .map(|value| load_names_in_expr(value.as_ref()))
            .unwrap_or_default(),
        _ => HashSet::new(),
    }
}

pub(super) fn load_names_in_terminator(terminator: &Terminator) -> HashSet<String> {
    match terminator {
        Terminator::BrIf { test, .. } => load_names_in_expr(test),
        Terminator::BrTable { index, .. } => load_names_in_expr(index),
        Terminator::Raise(raise_stmt) => {
            let mut names = HashSet::new();
            if let Some(exc) = raise_stmt.exc.as_ref() {
                names.extend(load_names_in_expr(exc.as_ref()));
            }
            if let Some(cause) = raise_stmt.cause.as_ref() {
                names.extend(load_names_in_expr(cause.as_ref()));
            }
            names
        }
        Terminator::TryJump { .. } => HashSet::new(),
        Terminator::Yield { value, .. } => {
            value.as_ref().map(load_names_in_expr).unwrap_or_default()
        }
        Terminator::Ret(Some(value)) => load_names_in_expr(value),
        Terminator::Jump(_) | Terminator::Ret(None) => HashSet::new(),
    }
}

pub(super) fn assigned_names_in_stmt(stmt: &Stmt) -> HashSet<String> {
    let mut names = HashSet::new();
    match stmt {
        // TODO(#2): model `del` as a kill-set in BB liveness instead of only
        // tracking defs. Without kills, deleted locals can be threaded across
        // block boundaries and incorrectly remain live.
        Stmt::Assign(assign) => {
            for target in &assign.targets {
                collect_assigned_names(target, &mut names);
            }
        }
        Stmt::If(if_stmt) => {
            for stmt in &if_stmt.body.body {
                names.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
            for clause in &if_stmt.elif_else_clauses {
                for stmt in &clause.body.body {
                    names.extend(assigned_names_in_stmt(stmt.as_ref()));
                }
            }
        }
        Stmt::While(while_stmt) => {
            for stmt in &while_stmt.body.body {
                names.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
            for stmt in &while_stmt.orelse.body {
                names.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
        }
        Stmt::For(for_stmt) => {
            collect_assigned_names(for_stmt.target.as_ref(), &mut names);
            for stmt in &for_stmt.body.body {
                names.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
            for stmt in &for_stmt.orelse.body {
                names.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
        }
        Stmt::Try(try_stmt) => {
            for stmt in &try_stmt.body.body {
                names.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
            for handler in &try_stmt.handlers {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                for stmt in &handler.body.body {
                    names.extend(assigned_names_in_stmt(stmt.as_ref()));
                }
            }
            for stmt in &try_stmt.orelse.body {
                names.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
            for stmt in &try_stmt.finalbody.body {
                names.extend(assigned_names_in_stmt(stmt.as_ref()));
            }
        }
        Stmt::FunctionDef(func_def) => {
            names.insert(func_def.name.id.to_string());
        }
        _ => {}
    }
    names
}

pub(super) fn collect_assigned_names(target: &Expr, names: &mut HashSet<String>) {
    match target {
        Expr::Name(name) => {
            names.insert(name.id.to_string());
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                collect_assigned_names(elt, names);
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                collect_assigned_names(elt, names);
            }
        }
        Expr::Starred(starred) => collect_assigned_names(starred.value.as_ref(), names),
        _ => {}
    }
}
