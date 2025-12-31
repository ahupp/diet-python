use std::{collections::HashSet, mem::take};

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
    class_locals: HashSet<String>,
    locals: HashSet<String>,
    params: HashSet<String>,
}

impl Transformer for MethodTransformer {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if matches!(stmt, Stmt::FunctionDef(_)) {
            return;
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
            Expr::Attribute(ast::ExprAttribute { attr: _, .. }) => {
                walk_expr(self, expr);
                return;
            }
            Expr::Name(ast::ExprName { id, ctx, .. }) => {
                if matches!(ctx, ExprContext::Load) {
                    if let Some(prefix) = private_prefix(self.class_expr.as_str()) {
                        if id.as_str().starts_with(prefix.as_str())
                            && !self.locals.contains(id.as_str())
                        {
                            *expr = py_expr!(
                                "_dp_class_ns.{storage_name:id}",
                                storage_name = id.as_str(),
                            );
                            return;
                        }
                    }
                    if id == "__class__" {
                        *expr = py_expr!("{cls:id}", cls = self.class_expr.as_str());
                    } else if self.class_locals.contains(id.as_str())
                        && !self.locals.contains(id.as_str())
                        && !self.params.contains(id.as_str())
                    {
                        *expr = py_expr!(
                            "__dp__.global_(globals(), {name:literal})",
                            name = id.as_str()
                        );
                    } else if id.as_str() == self.method_name && !self.params.contains(id.as_str())
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

fn private_prefix(class_name: &str) -> Option<String> {
    let mut class_name = class_name;
    while class_name.starts_with('_') {
        class_name = &class_name[1..];
    }
    if class_name.is_empty() {
        return None;
    }
    Some(format!("_{}__", class_name))
}

pub fn rewrite_method(
    func_def: &mut ast::StmtFunctionDef,
    class_name: &str,
    original_method_name: &str,
    class_locals: &HashSet<String>,
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

    let scope = rewriter.context().analyze_function_scope(func_def);

    let params: HashSet<String> = scope.params.iter().cloned().collect();
    let mut transformer = MethodTransformer {
        class_expr: class_name.to_string(),
        first_arg,
        method_name: original_method_name.to_string(),
        class_locals: class_locals.clone(),
        locals: scope.locals.clone(),
        params,
    };
    for stmt in &mut func_def.body {
        walk_stmt(&mut transformer, stmt);
    }

    let body = take(&mut func_def.body);
    func_def.body =
        rewriter.with_function_scope(scope, move |rewriter| rewriter.rewrite_block(body));
}
