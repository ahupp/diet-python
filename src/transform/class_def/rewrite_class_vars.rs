use std::{borrow::Cow, collections::HashSet, mem};

use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};

use crate::template::py_stmt_single;
use crate::{
    body_transform::{walk_expr, walk_stmt, Transformer},
    py_expr, py_stmt,
};

pub struct ClassVarRenamer {
    class_name: String,
    stored: HashSet<String>,
    globals: HashSet<String>,
    nonlocals: HashSet<String>,
    pending: HashSet<String>,
    assignment_targets: HashSet<String>,
    emitted: Vec<Stmt>,
}

impl ClassVarRenamer {
    pub fn new(class_name: &str) -> Self {
        Self {
            class_name: class_name.to_string(),
            stored: HashSet::new(),
            globals: HashSet::new(),
            nonlocals: HashSet::new(),
            pending: HashSet::new(),
            assignment_targets: HashSet::new(),
            emitted: Vec::new(),
        }
    }

    fn should_rewrite(&self, name: &str) -> bool {
        !self.globals.contains(name) && !self.nonlocals.contains(name) && !name.starts_with("_dp_")
    }

    fn emit_after(&mut self, stmts: Vec<Stmt>) {
        self.emitted.extend(stmts);
    }

    fn storage_name<'a>(&'a self, name: &'a str) -> Cow<'a, str> {
        if let Some(mangled) = mangle_private_name(self.class_name.as_str(), name) {
            Cow::Owned(mangled)
        } else {
            Cow::Borrowed(name)
        }
    }
}

impl Transformer for ClassVarRenamer {
    fn visit_body(&mut self, body: &mut Vec<Stmt>) {
        let mut added_pending = Vec::new();
        // We collect pending method names up front so loads inside the function
        // body are treated as globals until the method is published back into
        // the class namespace. This mirrors Python's class scope behavior
        // where the def's name is only bound after the body executes.
        for stmt in body.iter() {
            if let Stmt::FunctionDef(ast::StmtFunctionDef { name, .. }) = stmt {
                let function_name = name.id.as_str();
                if self.should_rewrite(function_name) {
                    let inserted = self.pending.insert(function_name.to_string());
                    if inserted {
                        added_pending.push(function_name.to_string());
                    }
                }
            }
        }

        let mut new_body = Vec::with_capacity(body.len());
        for mut stmt in mem::take(body) {
            self.visit_stmt(&mut stmt);
            new_body.push(stmt);
            if !self.emitted.is_empty() {
                new_body.append(&mut self.emitted);
            }
        }
        *body = new_body;

        for name in added_pending {
            self.pending.remove(&name);
        }
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
                    // `stored` tracks which names live in `_dp_class_ns`, so we
                    // record the function before rewriting future loads.
                    self.stored.insert(original_name.clone());
                    // Mark this def as pending so the function body can still
                    // see the global binding while decorators run.
                    self.pending.insert(original_name.clone());
                    let storage_name = self.storage_name(original_name.as_str());
                    self.emit_after(py_stmt!(
                        "_dp_class_ns.{storage_name:id} = {name:id}",
                        storage_name = storage_name.as_ref(),
                        name = original_name.as_str(),
                    ));
                    // Once the helper executes the pending state is cleared,
                    // restoring class-scope loads to target `_dp_class_ns`.
                    self.pending.remove(original_name.as_str());
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
                    let name = id.as_str();
                    if self.should_rewrite(name) {
                        // Mark stored after visiting value, in case you have x = x, where rhs is global
                        // and must still resolve to the outer scope before we rewrite the target.
                        self.stored.insert(name.to_string());
                        let storage_name = self.storage_name(name);
                        *target = py_expr!(
                            "_dp_class_ns.{storage_name:id}",
                            storage_name = storage_name.as_ref(),
                        );
                    }
                } else {
                    walk_stmt(self, stmt);
                    return;
                }
            }
            Stmt::Delete(ast::StmtDelete { targets, .. }) => {
                if targets.len() == 1 {
                    if let Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                        let name = id.as_str();
                        if self.should_rewrite(name) {
                            // Deletes must update `_dp_class_ns` directly so the
                            // synthesized class dict matches Python's runtime behavior.
                            self.stored.remove(name);
                            self.pending.remove(name);
                            let storage_name = self.storage_name(name);
                            *stmt = py_stmt_single(py_stmt!(
                                "del _dp_class_ns.{storage_name:id}",
                                storage_name = storage_name.as_ref(),
                            ));
                            return;
                        }
                    }
                }
                walk_stmt(self, stmt);
                return;
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
            Stmt::AugAssign(_) => {
                panic!("augassign should be rewritten to assign");
            }
            Stmt::AnnAssign(_) => {
                walk_stmt(self, stmt);
                return;
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
                            // After publication we rewrite loads to hit the
                            // explicit `_dp_class_ns` entry so attributes live
                            // on the constructed class.
                            let storage_name = self.storage_name(name_str);
                            *expr = py_expr!(
                                "_dp_class_ns.{storage_name:id}",
                                storage_name = storage_name.as_ref(),
                            );
                        } else if self.pending.contains(name_str)
                            && !self.assignment_targets.contains(name_str)
                        {
                            // While the method is pending we force loads to
                            // resolve as globals, matching Python's class body
                            // semantics where the function name is not yet
                            // rebound in the local namespace.
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

pub(crate) fn mangle_private_name(class_name: &str, attr: &str) -> Option<String> {
    if !attr.starts_with("__") || attr.ends_with("__") {
        return None;
    }

    let mut class_name = class_name;
    while class_name.starts_with('_') {
        class_name = &class_name[1..];
    }

    if class_name.is_empty() {
        return None;
    }

    Some(format!("_{}{}", class_name, attr))
}
