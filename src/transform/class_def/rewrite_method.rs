use std::{collections::HashSet, mem::take};

use ruff_python_ast::{self as ast, Expr, ExprContext, Stmt};

use crate::{
    body_transform::{walk_expr, walk_stmt, Transformer},
    py_expr,
    transform::driver::ExprRewriter,
};

struct MethodTransformer {
    first_arg: Option<String>,
    method_name: String,
    class_locals: HashSet<String>,
    locals: HashSet<String>,
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

                    *expr = match &self.first_arg {
                        Some(arg) => {
                            py_expr!("__dp__.super_(__class__, {arg:id})", arg = arg.as_str())
                        }
                        None => py_expr!("__dp__.super_(__class__, None)"),
                    };
                }
            }
            Expr::Name(ast::ExprName { id, ctx, .. }) => {
                if matches!(ctx, ExprContext::Load) {
                    if self.class_locals.contains(id.as_str())
                        && !self.locals.contains(id.as_str())
                    {
                        *expr = py_expr!(
                            "__dp__.global_(globals(), {name:literal})",
                            name = id.as_str()
                        );
                    } else if id.as_str() == self.method_name
                        && !self.locals.contains(id.as_str())
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
    class_qualname: &str,
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

    let mut scope = rewriter.context().analyze_function_scope(func_def);
    scope.qualname = format!("{class_qualname}.{original_method_name}");

    let locals = scope.local_names();
    let mut transformer = MethodTransformer {
        first_arg,
        method_name: original_method_name.to_string(),
        class_locals: class_locals.clone(),
        locals,
    };
    for stmt in &mut func_def.body {
        transformer.visit_stmt(stmt);
    }

    let body = take(&mut func_def.body);
    func_def.body =
        rewriter.with_function_scope(scope, move |rewriter| rewriter.rewrite_block(body));
}
