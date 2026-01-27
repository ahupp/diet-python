

use ruff_python_ast::{self as ast,Expr, ExprContext, Stmt};

use crate::{
    body_transform::{Transformer, walk_expr, walk_stmt},
    py_expr, py_stmt,
    transform::{util::is_noarg_call},
};

struct MethodRewriteSuperClasscell {
    first_arg: Option<String>,
    needs_class_cell: bool,
}

impl Transformer for MethodRewriteSuperClasscell {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(_) => return,
            Stmt::Delete(ast::StmtDelete { targets, .. }) => {
                assert!(targets.len() == 1);
                if let Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                    if id.as_str() == "__class__" {
                        *stmt = py_stmt!("del __classcell__.cell_contents").remove(0);
                        self.needs_class_cell = true;
                        return;
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
            Expr::Name(ast::ExprName { id, .. }) => {
                if id.as_str() == "__class__"
                {
                    self.needs_class_cell = true;
                    *expr = py_expr!("__classcell__.cell_contents");
                }
                return;
            }
            _ => {}
        }

        walk_expr(self, expr);
    }
}


pub fn rewrite_methods_in_class_body(
    class_def: &mut ast::StmtClassDef,
) -> bool {
    let mut rewriter = MethodRewriter {
        needs_class_cell: false,
    };
    rewriter.visit_body(&mut class_def.body);
    rewriter.needs_class_cell
}


struct MethodRewriter {
    needs_class_cell: bool,
}

impl Transformer for MethodRewriter {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(func_def) => {
                self.needs_class_cell |= rewrite_method(
                    func_def,
                );
            }
            Stmt::ClassDef(_) => {}
            _ => walk_stmt(self, stmt),
        }
    }
}


fn rewrite_method(
    func_def: &mut ast::StmtFunctionDef,
) -> bool {

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


    let mut transformer = MethodRewriteSuperClasscell {
        first_arg,
        needs_class_cell: false,
    };
    for stmt in &mut func_def.body {
        transformer.visit_stmt(stmt);
    }
    transformer.needs_class_cell
}
