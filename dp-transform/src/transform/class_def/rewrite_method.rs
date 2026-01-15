use std::{collections::HashSet, mem::take};

use ruff_python_ast::{self as ast, name::Name, Expr, ExprContext, Stmt};

use crate::{
    body_transform::{walk_expr, walk_stmt, Transformer},
    py_expr, py_stmt,
    template::py_stmt_single,
    transform::driver::ExprRewriter,
};

struct MethodTransformer {
    first_arg: Option<String>,
    locals: HashSet<String>,
    globals: HashSet<String>,
    needs_class_cell: bool,
    class_cell_name: String,
}

impl Transformer for MethodTransformer {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(_) => return,
            Stmt::Nonlocal(ast::StmtNonlocal { names, .. }) => {
                let had_class = names.iter().any(|name| name.id.as_str() == "__class__");
                if had_class && !self.globals.contains("__class__") {
                    self.needs_class_cell = true;
                    for name in names.iter_mut() {
                        if name.id.as_str() == "__class__" {
                            name.id = Name::new(self.class_cell_name.as_str());
                        }
                    }
                }
                return;
            }
            Stmt::Assign(ast::StmtAssign { targets, value, .. }) => {
                if targets.len() == 1 {
                    if let Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                        if id.as_str() == "__class__"
                            && !self.locals.contains("__class__")
                            && !self.globals.contains("__class__")
                        {
                            self.needs_class_cell = true;
                            self.visit_expr(value);
                            *stmt = py_stmt_single(py_stmt!(
                                "{class_cell:id} = {value:expr}",
                                class_cell = self.class_cell_name.as_str(),
                                value = value.clone()
                            ));
                            return;
                        }
                    }
                }
            }
            Stmt::Delete(ast::StmtDelete { targets, .. }) => {
                if targets.len() == 1 {
                    if let Expr::Name(ast::ExprName { id, .. }) = &targets[0] {
                        if id.as_str() == "__class__"
                            && !self.locals.contains("__class__")
                            && !self.globals.contains("__class__")
                        {
                            self.needs_class_cell = true;
                            *stmt = py_stmt_single(py_stmt!(
                                "{class_cell:id} = __dp__.empty_classcell()",
                                class_cell = self.class_cell_name.as_str(),
                            ));
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
                    self.needs_class_cell = true;

                    *expr = match &self.first_arg {
                        Some(arg) => py_expr!(
                            "__dp__.call_super(super, {class_cell:id}, {arg:id})",
                            class_cell = self.class_cell_name.as_str(),
                            arg = arg.as_str()
                        ),
                        None => py_expr!("__dp__.call_super_noargs(super)"),
                    };
                }
            }
            Expr::Name(ast::ExprName { id, ctx, .. }) => {
                if matches!(ctx, ExprContext::Load) {
                    if id.as_str() == "__class__"
                        && !self.locals.contains("__class__")
                        && !self.globals.contains("__class__")
                    {
                        self.needs_class_cell = true;
                        *expr = py_expr!(
                            "__dp__.class_cell_value({class_cell:id})",
                            class_cell = self.class_cell_name.as_str()
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
    rewriter: &mut ExprRewriter,
    class_cell_name: &str,
) -> bool {
    let func_name = func_def.name.id.to_string();
    let Some(original_method_name) = func_name.strip_prefix("_dp_fn_") 
    else {
        // Internal function, not actually a method
        return false;
    };

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
    let globals = scope.global_names();
    let mut transformer = MethodTransformer {
        first_arg,
        locals,
        globals,
        needs_class_cell: false,
        class_cell_name: class_cell_name.to_string(),
    };
    for stmt in &mut func_def.body {
        transformer.visit_stmt(stmt);
    }

    let body = take(&mut func_def.body);
    func_def.body =
        rewriter.with_function_scope(scope, move |rewriter| rewriter.rewrite_block(body));
    transformer.needs_class_cell
}
