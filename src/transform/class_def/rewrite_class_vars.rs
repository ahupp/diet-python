use std::collections::HashSet;

use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};

use crate::{
    body_transform::{walk_expr, walk_stmt, Transformer},
    py_expr,
};

pub struct ClassVarRenamer {
    stored: HashSet<String>,
    globals: HashSet<String>,
    nonlocals: HashSet<String>,
    pending: HashSet<String>,
    assignment_targets: HashSet<String>,
}

impl ClassVarRenamer {
    pub fn new() -> Self {
        Self {
            stored: HashSet::new(),
            globals: HashSet::new(),
            nonlocals: HashSet::new(),
            pending: HashSet::new(),
            assignment_targets: HashSet::new(),
        }
    }

    fn should_rewrite(&self, name: &str) -> bool {
        !self.globals.contains(name) && !self.nonlocals.contains(name) && !name.starts_with("_dp_")
    }
}

impl Transformer for ClassVarRenamer {
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
                    self.pending.insert(original_name);
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
                // don't traverse into function body
            }
            Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                assert!(targets.len() == 1, "assign should have a single target");
                let target = targets.first_mut().unwrap();
                if let Expr::Name(ast::ExprName { id, .. }) = target {
                    self.visit_expr(value);
                    // Mark stored after visiting value, in case you have x = x, where rhs is global
                    self.stored.insert(id.as_str().to_string());
                    *target = py_expr!("_dp_class_ns[{name:literal}]", name = id.as_str());
                } else {
                    walk_stmt(self, stmt);
                    return;
                }
            }
            Stmt::Global(ast::StmtGlobal { names, .. }) => {
                for name in names {
                    self.globals.insert(name.id.to_string());
                }
            }
            Stmt::Nonlocal(ast::StmtNonlocal { names, .. }) => {
                for name in names {
                    self.nonlocals.insert(name.id.to_string());
                }
            }
            Stmt::AnnAssign(_) | Stmt::AugAssign(_) => {
                panic!("augassign should be rewritten to assign");
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Name(ast::ExprName { id, ctx, .. }) = expr {
            match ctx {
                ExprContext::Load => {
                    let name = id.as_str().to_string();
                    let name_str = name.as_str();
                    if self.should_rewrite(name_str) {
                        if self.stored.contains(name_str) && !self.pending.contains(name_str) {
                            *expr = py_expr!("_dp_class_ns[{name:literal}]", name = name_str);
                        } else if self.pending.contains(name_str)
                            && !self.assignment_targets.contains(name_str)
                        {
                            *expr = py_expr!(
                                "__dp__.global_(globals(), {name:literal})",
                                name = name_str,
                            );
                        }
                    }
                }
                _ => {}
            }
        }
        walk_expr(self, expr);
    }
}
