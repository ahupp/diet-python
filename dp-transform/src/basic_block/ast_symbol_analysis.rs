use crate::basic_block::ast_to_ast::body::suite_ref;
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

pub(crate) fn load_names_in_expr(expr: &Expr) -> HashSet<String> {
    let mut expr = expr.clone();
    let mut collector = LoadNameCollector::default();
    collector.visit_expr(&mut expr);
    collector.names
}

pub(crate) fn assigned_names_in_stmt(stmt: &Stmt) -> HashSet<String> {
    let mut names = HashSet::new();
    match stmt {
        Stmt::Assign(assign) => {
            collect_named_expr_target_names_in_expr(assign.value.as_ref(), &mut names);
            for target in &assign.targets {
                collect_assigned_names(target, &mut names);
            }
        }
        Stmt::If(if_stmt) => {
            collect_named_expr_target_names_in_expr(if_stmt.test.as_ref(), &mut names);
            for stmt in suite_ref(&if_stmt.body) {
                names.extend(assigned_names_in_stmt(stmt));
            }
            for clause in &if_stmt.elif_else_clauses {
                if let Some(test) = clause.test.as_ref() {
                    collect_named_expr_target_names_in_expr(test, &mut names);
                }
                for stmt in suite_ref(&clause.body) {
                    names.extend(assigned_names_in_stmt(stmt));
                }
            }
        }
        Stmt::While(while_stmt) => {
            collect_named_expr_target_names_in_expr(while_stmt.test.as_ref(), &mut names);
            for stmt in suite_ref(&while_stmt.body) {
                names.extend(assigned_names_in_stmt(stmt));
            }
            for stmt in suite_ref(&while_stmt.orelse) {
                names.extend(assigned_names_in_stmt(stmt));
            }
        }
        Stmt::For(for_stmt) => {
            collect_named_expr_target_names_in_expr(for_stmt.iter.as_ref(), &mut names);
            collect_assigned_names(for_stmt.target.as_ref(), &mut names);
            for stmt in suite_ref(&for_stmt.body) {
                names.extend(assigned_names_in_stmt(stmt));
            }
            for stmt in suite_ref(&for_stmt.orelse) {
                names.extend(assigned_names_in_stmt(stmt));
            }
        }
        Stmt::Try(try_stmt) => {
            for stmt in suite_ref(&try_stmt.body) {
                names.extend(assigned_names_in_stmt(stmt));
            }
            for handler in &try_stmt.handlers {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                if let Some(type_) = handler.type_.as_ref() {
                    collect_named_expr_target_names_in_expr(type_.as_ref(), &mut names);
                }
                for stmt in suite_ref(&handler.body) {
                    names.extend(assigned_names_in_stmt(stmt));
                }
            }
            for stmt in suite_ref(&try_stmt.orelse) {
                names.extend(assigned_names_in_stmt(stmt));
            }
            for stmt in suite_ref(&try_stmt.finalbody) {
                names.extend(assigned_names_in_stmt(stmt));
            }
        }
        Stmt::Expr(expr_stmt) => {
            collect_named_expr_target_names_in_expr(expr_stmt.value.as_ref(), &mut names);
        }
        Stmt::FunctionDef(func_def) => {
            names.insert(func_def.name.id.to_string());
        }
        Stmt::Return(ret) => {
            if let Some(value) = ret.value.as_ref() {
                collect_named_expr_target_names_in_expr(value.as_ref(), &mut names);
            }
        }
        Stmt::Raise(raise_stmt) => {
            if let Some(exc) = raise_stmt.exc.as_ref() {
                collect_named_expr_target_names_in_expr(exc.as_ref(), &mut names);
            }
            if let Some(cause) = raise_stmt.cause.as_ref() {
                collect_named_expr_target_names_in_expr(cause.as_ref(), &mut names);
            }
        }
        _ => {}
    }
    names
}

#[derive(Default)]
struct NamedExprTargetCollector {
    names: HashSet<String>,
}

impl Transformer for NamedExprTargetCollector {
    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Named(ast::ExprNamed { target, value, .. }) = expr {
            collect_assigned_names(target.as_ref(), &mut self.names);
            self.visit_expr(value.as_mut());
            return;
        }
        walk_expr(self, expr);
    }
}

fn collect_named_expr_target_names_in_expr(expr: &Expr, names: &mut HashSet<String>) {
    let mut expr = expr.clone();
    let mut collector = NamedExprTargetCollector::default();
    collector.visit_expr(&mut expr);
    names.extend(collector.names);
}

pub(crate) fn collect_assigned_names(target: &Expr, names: &mut HashSet<String>) {
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
