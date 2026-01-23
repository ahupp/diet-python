use std::{mem::take};

use ruff_python_ast::{self as ast,Expr, ExprContext, Stmt};

use crate::{
    body_transform::{Transformer, walk_expr, walk_stmt},
    py_expr, py_stmt,
    transform::{driver::ExprRewriter, util::is_noarg_call},
};

struct MethodTransformer {
    first_arg: Option<String>,
    needs_class_cell: bool,
}

impl Transformer for MethodTransformer {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(_) => return,
            Stmt::Delete(ast::StmtDelete { targets, .. }) => {
                if targets.len() == 1 {
                    if let Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                        if id.as_str() == "__class__" {
                            *stmt = py_stmt!("del __classcell__.cell_contents").remove(0);
                            self.needs_class_cell = true;
                            return;
                        }
                    }
                }
            }
            _ => {}
        }

        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Call(_) => {

                if is_noarg_call("super", expr) {
                    self.needs_class_cell = true;
                    *expr = match &self.first_arg {
                        Some(arg) => py_expr!(
                            "__dp__.call_super(super, __classcell__, {arg:id})",
                            arg = arg.as_str()
                        ),
                        None => py_expr!("__dp__.call_super_noargs(super)"),
                    };
                }
            }
            Expr::Name(ast::ExprName { id, ctx, .. }) => {
                if matches!(ctx, ExprContext::Load) {
                    if id.as_str() == "__class__"
                    {
                        self.needs_class_cell = true;
                        *expr = py_expr!("__classcell__.cell_contents");
                    }
                }
                return;
            }
            _ => {}
        }

        walk_expr(self, expr);
    }
}


pub fn rewrite_methods_in_class_body(
    body: &mut Vec<Stmt>,
    class_qualname: &str,
    rewriter: &mut ExprRewriter,
) -> bool {
    let mut rewriter = MethodRewriter {
        class_qualname: class_qualname.to_string(),
        expr_rewriter: rewriter,
        needs_class_cell: false,
    };
    rewriter.visit_body(body);
    rewriter.needs_class_cell
}


struct MethodRewriter<'a> {
    class_qualname: String,
    expr_rewriter: &'a mut ExprRewriter,
    needs_class_cell: bool,
}

impl<'a> Transformer for MethodRewriter<'a> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(func_def) => {
                assert!(
                    func_def.decorator_list.is_empty(),
                    "decorators should be gone by now"
                );
                self.needs_class_cell |= rewrite_method(
                    func_def,
                    &self.class_qualname,
                    self.expr_rewriter,
                );
            }
            Stmt::ClassDef(_) => {}
            _ => walk_stmt(self, stmt),
        }
    }
}


pub fn rewrite_method(
    func_def: &mut ast::StmtFunctionDef,
    class_qualname: &str,
    rewriter: &mut ExprRewriter,
) -> bool {
    let func_name = func_def.name.id.to_string();
    if let Some(_) = func_name.strip_prefix("_dp_fn_") {
        return false;
    }

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

    let mut transformer = MethodTransformer {
        first_arg,
        needs_class_cell: false,
    };
    for stmt in &mut func_def.body {
        transformer.visit_stmt(stmt);
    }
    scope.qualname = format!("{class_qualname}.{func_name}");

    let body = take(&mut func_def.body);
    func_def.body =
        rewriter.with_scope(scope, move |rewriter| rewriter.rewrite_block(body));
    transformer.needs_class_cell
}
