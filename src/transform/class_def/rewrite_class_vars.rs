use std::{collections::HashSet, mem};

use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};

use crate::template::py_stmt_single;
use crate::{
    body_transform::{walk_expr, walk_stmt, Transformer},
    py_expr, py_stmt,
    transform::context::ScopeInfo,
};

pub fn rewrite_class_scope(body: &mut Vec<Stmt>, scope: ScopeInfo, enclosing: HashSet<String>) {
    let mut rewriter = ClassScopeRewriter::new(scope, enclosing);
    rewriter.visit_body(body);
}

struct ClassScopeRewriter {
    stored: HashSet<String>,
    globals: HashSet<String>,
    nonlocals: HashSet<String>,
    enclosing: HashSet<String>,
    emitted: Vec<Stmt>,
}

impl ClassScopeRewriter {
    fn new(scope: ScopeInfo, enclosing: HashSet<String>) -> Self {
        let globals = scope.global_names();
        let nonlocals = scope.nonlocal_names();
        Self {
            stored: HashSet::new(),
            globals,
            nonlocals,
            enclosing,
            emitted: Vec::new(),
        }
    }

    fn should_rewrite(&self, name: &str) -> bool {
        !self.globals.contains(name)
            && !self.nonlocals.contains(name)
            && !name.starts_with("_dp_")
            && !matches!(name, "__dp__" | "globals" | "locals" | "vars")
    }

    fn emit_after(&mut self, stmts: Vec<Stmt>) {
        self.emitted.extend(stmts);
    }
}

impl Transformer for ClassScopeRewriter {
    fn visit_body(&mut self, body: &mut Vec<Stmt>) {
        let mut new_body = Vec::with_capacity(body.len());
        for mut stmt in mem::take(body) {
            self.visit_stmt(&mut stmt);
            new_body.push(stmt);
            if !self.emitted.is_empty() {
                new_body.append(&mut self.emitted);
            }
        }
        *body = new_body;
    }

    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(ast::StmtFunctionDef {
                name,
                decorator_list,
                parameters,
                returns,
                type_params,
                ..
            }) => {
                let original_name = name.id.as_str().to_string();
                if self.should_rewrite(original_name.as_str()) {
                    self.stored.insert(original_name.clone());
                    self.emit_after(py_stmt!(
                        "_dp_class_ns.{storage_name:id} = {name:id}",
                        storage_name = original_name.as_str(),
                        name = original_name.as_str(),
                    ));
                }

                for decorator in decorator_list {
                    self.visit_decorator(decorator);
                }
                if let Some(type_params) = type_params {
                    self.visit_type_params(type_params);
                }
                self.visit_parameters(parameters);
                if let Some(expr) = returns {
                    self.visit_annotation(expr);
                }
            }
            Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                assert!(targets.len() == 1, "assign should have a single target");
                let target = targets.first_mut().unwrap();
                if let Expr::Name(ast::ExprName { id, .. }) = target {
                    self.visit_expr(value);
                    let name = id.as_str();
                    if self.should_rewrite(name) {
                        self.stored.insert(name.to_string());
                        *target = py_expr!("_dp_class_ns.{storage_name:id}", storage_name = name,);
                    }
                } else {
                    walk_stmt(self, stmt);
                    return;
                }
            }
            Stmt::Delete(ast::StmtDelete { targets, .. }) => {
                assert!(targets.len() == 1);
                if let Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                    let name = id.as_str();
                    if self.should_rewrite(name) {
                        self.stored.remove(name);
                        *stmt = py_stmt_single(py_stmt!(
                            "del _dp_class_ns.{storage_name:id}",
                            storage_name = name,
                        ));
                        return;
                    }
                }
                walk_stmt(self, stmt);
                return;
            }
            Stmt::Global(_) | Stmt::Nonlocal(_) => {}
            Stmt::AugAssign(_) => {
                panic!("augassign should be rewritten to assign");
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Call(ast::ExprCall {
            func, arguments, ..
        }) = expr
        {
            if let Expr::Name(ast::ExprName { id, .. }) = func.as_ref() {
                if id.as_str() == "vars"
                    && arguments.args.is_empty()
                    && arguments.keywords.is_empty()
                {
                    *expr = py_expr!("_dp_class_ns._namespace");
                    return;
                }
            }
        }
        if let Expr::Name(ast::ExprName { id, ctx, .. }) = expr {
            if matches!(ctx, ExprContext::Load) {
                let name = id.as_str().to_string();
                let name_str = name.as_str();
                if !self.should_rewrite(name_str) {
                    return;
                }
                if self.stored.contains(name_str) {
                    *expr = py_expr!("_dp_class_ns.{storage_name:id}", storage_name = name_str,);
                } else if self.enclosing.contains(name_str) {
                    return;
                } else {
                    *expr = py_expr!(
                        "__dp__.class_lookup(_dp_class_ns, globals(), {name:literal})",
                        name = name_str,
                    );
                }
            }
        }
        walk_expr(self, expr);
    }
}
