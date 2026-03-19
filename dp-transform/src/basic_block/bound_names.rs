use crate::basic_block::ast_symbol_analysis::collect_assigned_names;
use crate::basic_block::ast_to_ast::body::suite_ref;
use ruff_python_ast::{self as ast, Stmt};
use std::collections::HashSet;

pub(crate) fn collect_bound_names(stmts: &[Box<Stmt>]) -> HashSet<String> {
    let mut names = HashSet::new();
    for stmt in stmts {
        collect_bound_names_in_stmt(stmt.as_ref(), &mut names);
    }
    names
}

fn collect_bound_names_in_stmt(stmt: &Stmt, names: &mut HashSet<String>) {
    match stmt {
        Stmt::Assign(assign) => {
            for target in &assign.targets {
                collect_assigned_names(target, names);
            }
        }
        Stmt::AugAssign(aug) => collect_assigned_names(aug.target.as_ref(), names),
        Stmt::AnnAssign(ann) => collect_assigned_names(ann.target.as_ref(), names),
        Stmt::For(for_stmt) => {
            collect_assigned_names(for_stmt.target.as_ref(), names);
            for child in suite_ref(&for_stmt.body) {
                collect_bound_names_in_stmt(child.as_ref(), names);
            }
            for child in suite_ref(&for_stmt.orelse) {
                collect_bound_names_in_stmt(child.as_ref(), names);
            }
        }
        Stmt::While(while_stmt) => {
            for child in suite_ref(&while_stmt.body) {
                collect_bound_names_in_stmt(child.as_ref(), names);
            }
            for child in suite_ref(&while_stmt.orelse) {
                collect_bound_names_in_stmt(child.as_ref(), names);
            }
        }
        Stmt::If(if_stmt) => {
            for child in suite_ref(&if_stmt.body) {
                collect_bound_names_in_stmt(child.as_ref(), names);
            }
            for clause in &if_stmt.elif_else_clauses {
                for child in suite_ref(&clause.body) {
                    collect_bound_names_in_stmt(child.as_ref(), names);
                }
            }
        }
        Stmt::Try(try_stmt) => {
            for child in suite_ref(&try_stmt.body) {
                collect_bound_names_in_stmt(child.as_ref(), names);
            }
            for handler in &try_stmt.handlers {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                if let Some(name) = handler.name.as_ref() {
                    names.insert(name.id.to_string());
                }
                for child in suite_ref(&handler.body) {
                    collect_bound_names_in_stmt(child.as_ref(), names);
                }
            }
            for child in suite_ref(&try_stmt.orelse) {
                collect_bound_names_in_stmt(child.as_ref(), names);
            }
            for child in suite_ref(&try_stmt.finalbody) {
                collect_bound_names_in_stmt(child.as_ref(), names);
            }
        }
        Stmt::With(with_stmt) => {
            for item in &with_stmt.items {
                if let Some(optional_vars) = item.optional_vars.as_ref() {
                    collect_assigned_names(optional_vars.as_ref(), names);
                }
            }
            for child in suite_ref(&with_stmt.body) {
                collect_bound_names_in_stmt(child.as_ref(), names);
            }
        }
        Stmt::Delete(delete_stmt) => {
            for target in &delete_stmt.targets {
                collect_assigned_names(target, names);
            }
        }
        Stmt::FunctionDef(func_def) => {
            names.insert(func_def.name.id.to_string());
        }
        Stmt::ClassDef(class_def) => {
            names.insert(class_def.name.id.to_string());
        }
        _ => {}
    }
}

pub(crate) fn collect_explicit_global_or_nonlocal_names(stmts: &[Box<Stmt>]) -> HashSet<String> {
    let mut names = HashSet::new();
    for stmt in stmts {
        collect_explicit_global_or_nonlocal_names_in_stmt(stmt.as_ref(), &mut names);
    }
    names
}

fn collect_explicit_global_or_nonlocal_names_in_stmt(stmt: &Stmt, names: &mut HashSet<String>) {
    match stmt {
        Stmt::Global(global_stmt) => {
            for name in &global_stmt.names {
                names.insert(name.id.to_string());
            }
        }
        Stmt::Nonlocal(nonlocal_stmt) => {
            for name in &nonlocal_stmt.names {
                names.insert(name.id.to_string());
            }
        }
        Stmt::If(if_stmt) => {
            for child in suite_ref(&if_stmt.body) {
                collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
            }
            for clause in &if_stmt.elif_else_clauses {
                for child in suite_ref(&clause.body) {
                    collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
                }
            }
        }
        Stmt::While(while_stmt) => {
            for child in suite_ref(&while_stmt.body) {
                collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
            }
            for child in suite_ref(&while_stmt.orelse) {
                collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
            }
        }
        Stmt::For(for_stmt) => {
            for child in suite_ref(&for_stmt.body) {
                collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
            }
            for child in suite_ref(&for_stmt.orelse) {
                collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
            }
        }
        Stmt::Try(try_stmt) => {
            for child in suite_ref(&try_stmt.body) {
                collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
            }
            for handler in &try_stmt.handlers {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                for child in suite_ref(&handler.body) {
                    collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
                }
            }
            for child in suite_ref(&try_stmt.orelse) {
                collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
            }
            for child in suite_ref(&try_stmt.finalbody) {
                collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
            }
        }
        Stmt::With(with_stmt) => {
            for child in suite_ref(&with_stmt.body) {
                collect_explicit_global_or_nonlocal_names_in_stmt(child.as_ref(), names);
            }
        }
        _ => {}
    }
}
