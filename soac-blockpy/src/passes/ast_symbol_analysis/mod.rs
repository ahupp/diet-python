use crate::transformer::{walk_expr, walk_stmt, Transformer};
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

#[derive(Default)]
struct CurrentScopeLoadNameCollector {
    names: HashSet<String>,
}

impl Transformer for CurrentScopeLoadNameCollector {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
            other => walk_stmt(self, other),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Name(name) => {
                if matches!(name.ctx, ast::ExprContext::Load) {
                    self.names.insert(name.id.to_string());
                }
                walk_expr(self, expr);
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

pub(crate) fn collect_loaded_names(stmts: &[Stmt]) -> HashSet<String> {
    let mut body = stmts.to_vec();
    let mut collector = CurrentScopeLoadNameCollector::default();
    collector.visit_body(&mut body);
    collector.names
}

#[derive(Default)]
struct BoundNameCollector {
    names: HashSet<String>,
}

impl Transformer for BoundNameCollector {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Assign(assign) => {
                for target in &assign.targets {
                    collect_assigned_names(target, &mut self.names);
                }
                walk_stmt(self, stmt);
            }
            Stmt::AugAssign(aug) => {
                collect_assigned_names(aug.target.as_ref(), &mut self.names);
                walk_stmt(self, stmt);
            }
            Stmt::AnnAssign(ann) => {
                collect_assigned_names(ann.target.as_ref(), &mut self.names);
                walk_stmt(self, stmt);
            }
            Stmt::For(for_stmt) => {
                collect_assigned_names(for_stmt.target.as_ref(), &mut self.names);
                walk_stmt(self, stmt);
            }
            Stmt::With(with_stmt) => {
                for item in &with_stmt.items {
                    if let Some(optional_vars) = item.optional_vars.as_ref() {
                        collect_assigned_names(optional_vars.as_ref(), &mut self.names);
                    }
                }
                walk_stmt(self, stmt);
            }
            Stmt::Delete(delete_stmt) => {
                for target in &delete_stmt.targets {
                    collect_assigned_names(target, &mut self.names);
                }
                walk_stmt(self, stmt);
            }
            Stmt::Try(try_stmt) => {
                for handler in &try_stmt.handlers {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    if let Some(name) = handler.name.as_ref() {
                        self.names.insert(name.id.to_string());
                    }
                }
                walk_stmt(self, stmt);
            }
            Stmt::FunctionDef(func_def) => {
                self.names.insert(func_def.name.id.to_string());
            }
            Stmt::ClassDef(class_def) => {
                self.names.insert(class_def.name.id.to_string());
            }
            _ => walk_stmt(self, stmt),
        }
    }
}

pub(crate) fn collect_bound_names(stmts: &[Stmt]) -> HashSet<String> {
    let mut body = stmts.to_vec();
    let mut collector = BoundNameCollector::default();
    collector.visit_body(&mut body);
    collector.names
}

#[cfg(test)]
#[derive(Default)]
struct ExplicitGlobalOrNonlocalCollector {
    names: HashSet<String>,
}

#[cfg(test)]
impl Transformer for ExplicitGlobalOrNonlocalCollector {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Global(global_stmt) => {
                for name in &global_stmt.names {
                    self.names.insert(name.id.to_string());
                }
            }
            Stmt::Nonlocal(nonlocal_stmt) => {
                for name in &nonlocal_stmt.names {
                    self.names.insert(name.id.to_string());
                }
            }
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
            _ => walk_stmt(self, stmt),
        }
    }
}

#[cfg(test)]
pub(crate) fn collect_explicit_global_or_nonlocal_names(stmts: &[Stmt]) -> HashSet<String> {
    let mut body = stmts.to_vec();
    let mut collector = ExplicitGlobalOrNonlocalCollector::default();
    collector.visit_body(&mut body);
    collector.names
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

#[cfg(test)]
mod test;
