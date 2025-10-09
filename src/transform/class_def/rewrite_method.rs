use std::{collections::HashSet, mem::take};

use super::rewrite_class_vars::mangle_private_name;
use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};

use crate::{
    body_transform::{walk_expr, walk_stmt, Transformer},
    py_expr,
    transform::driver::ExprRewriter,
};

struct MethodTransformer {
    class_expr: String,
    first_arg: Option<String>,
    method_name: String,
    local_bindings: HashSet<String>,
}

impl MethodTransformer {
    fn collect_store_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Name(ast::ExprName { id, .. }) => {
                self.local_bindings.insert(id.to_string());
            }
            Expr::Tuple(ast::ExprTuple { elts, .. }) | Expr::List(ast::ExprList { elts, .. }) => {
                for elt in elts {
                    self.collect_store_expr(elt);
                }
            }
            Expr::Starred(ast::ExprStarred { value, .. }) => {
                self.collect_store_expr(value);
            }
            Expr::Attribute(ast::ExprAttribute { value, .. }) => {
                self.collect_store_expr(value);
            }
            Expr::Subscript(ast::ExprSubscript { value, slice, .. }) => {
                self.collect_store_expr(value);
                self.collect_store_expr(slice);
            }
            _ => {}
        }
    }
}

impl Transformer for MethodTransformer {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(_) => return,
            Stmt::Assign(ast::StmtAssign { targets, .. }) => {
                for target in targets {
                    self.collect_store_expr(target);
                }
            }
            Stmt::AnnAssign(ast::StmtAnnAssign { target, .. }) => {
                self.collect_store_expr(target);
            }
            Stmt::AugAssign(ast::StmtAugAssign { target, .. }) => {
                self.collect_store_expr(target);
            }
            Stmt::For(ast::StmtFor { target, .. }) => {
                self.collect_store_expr(target);
            }
            Stmt::With(ast::StmtWith { items, .. }) => {
                for item in items {
                    if let Some(optional_vars) = &item.optional_vars {
                        self.collect_store_expr(optional_vars);
                    }
                }
            }
            _ => {}
        }

        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Call(call) => {
                let is_zero_arg_super =
                    if let Expr::Name(ast::ExprName { id, .. }) = call.func.as_ref() {
                        id == "super"
                            && call.arguments.args.is_empty()
                            && call.arguments.keywords.is_empty()
                    } else {
                        false
                    };

                if is_zero_arg_super {
                    walk_expr(self, expr);

                    let replacement = match &self.first_arg {
                        Some(arg) => py_expr!(
                            "super({cls:id}, {arg:id})",
                            cls = self.class_expr.as_str(),
                            arg = arg.as_str()
                        ),
                        None => py_expr!("super({cls:id}, None)", cls = self.class_expr.as_str()),
                    };

                    *expr = replacement;
                } else {
                    walk_expr(self, expr);
                }
                return;
            }
            Expr::Attribute(ast::ExprAttribute { attr, .. }) => {
                if let Some(mangled) =
                    mangle_private_name(self.class_expr.as_str(), attr.id.as_str())
                {
                    attr.id = mangled.into();
                }

                walk_expr(self, expr);
                return;
            }
            Expr::Name(ast::ExprName { id, ctx, .. }) => {
                if matches!(ctx, ExprContext::Load) {
                    if let Some(mangled) =
                        mangle_private_name(self.class_expr.as_str(), id.as_str())
                    {
                        if !self.local_bindings.contains(id.as_str()) {
                            *expr = py_expr!(
                                "_dp_class_ns.{storage_name:id}",
                                storage_name = mangled.as_str(),
                            );
                            return;
                        }
                    }
                    if id == "__class__" {
                        *expr = py_expr!("{cls:id}", cls = self.class_expr.as_str());
                    } else if id.as_str() == self.method_name
                        && !self.local_bindings.contains(id.as_str())
                    {
                        *expr = py_expr!(
                            "__dp__.global_(globals(), {name:literal})",
                            name = self.method_name.as_str()
                        );
                    }
                }
                return;
            }
            _ => {}
        }

        walk_expr(self, expr);
    }
}

pub fn rewrite_method(
    func_def: &mut ast::StmtFunctionDef,
    class_name: &str,
    class_qualname: &str,
    original_method_name: &str,
    rewriter: &mut ExprRewriter,
) {
    let first_arg = func_def
        .parameters
        .posonlyargs
        .first()
        .map(|a| a.parameter.name.to_string())
        .or_else(|| {
            func_def
                .parameters
                .args
                .first()
                .map(|a| a.parameter.name.to_string())
        });

    let mut local_bindings: HashSet<String> = HashSet::new();
    for param in &func_def.parameters.posonlyargs {
        local_bindings.insert(param.name().to_string());
    }
    for param in &func_def.parameters.args {
        local_bindings.insert(param.name().to_string());
    }
    for param in &func_def.parameters.kwonlyargs {
        local_bindings.insert(param.name().to_string());
    }
    if let Some(param) = &func_def.parameters.vararg {
        local_bindings.insert(param.name.to_string());
    }
    if let Some(param) = &func_def.parameters.kwarg {
        local_bindings.insert(param.name.to_string());
    }

    let mut transformer = MethodTransformer {
        class_expr: class_name.to_string(),
        first_arg,
        method_name: original_method_name.to_string(),
        local_bindings,
    };
    for stmt in &mut func_def.body {
        walk_stmt(&mut transformer, stmt);
    }

    let method_qualname = format!("{class_qualname}.{original_method_name}");
    let body = take(&mut func_def.body);
    func_def.body = rewriter.with_class_scope(class_name, move |rewriter| {
        rewriter.with_function_scope(method_qualname, move |rewriter| {
            rewriter.rewrite_block(body)
        })
    });
}
