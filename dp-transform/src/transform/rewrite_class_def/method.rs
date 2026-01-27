

use ruff_python_ast::{self as ast,Expr, Stmt};

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
                    return;
                }
                if is_dp_call(expr, "call_super") || is_dp_call(expr, "call_super_noargs") {
                    self.needs_class_cell = true;
                }
            }
            Expr::Name(ast::ExprName { id, .. }) => {
                if id.as_str() == "__class__" {
                    self.needs_class_cell = true;
                    *expr = py_expr!("__classcell__.cell_contents");
                    return;
                }
                if id.as_str() == "__classcell__" {
                    self.needs_class_cell = true;
                    return;
                }
            }
            _ => {}
        }

        walk_expr(self, expr);
    }
}

fn is_dp_call(expr: &Expr, name: &str) -> bool {
    let Expr::Call(ast::ExprCall { func, .. }) = expr else {
        return false;
    };
    let Expr::Attribute(ast::ExprAttribute { value, attr, .. }) = func.as_ref() else {
        return false;
    };
    if attr.as_str() != name {
        return false;
    }
    matches!(
        value.as_ref(),
        Expr::Name(ast::ExprName { id, .. }) if id.as_str() == "__dp__"
    )
}


pub fn rewrite_explicit_super_classcell(
    class_def: &mut ast::StmtClassDef,
) -> bool {
    let mut rewriter = MethodExplicitSuperRewriter {
        needs_class_cell: false,
    };
    rewriter.visit_body(&mut class_def.body);
    rewriter.needs_class_cell
}


struct MethodExplicitSuperRewriter {
    needs_class_cell: bool,
}

impl Transformer for MethodExplicitSuperRewriter {
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
