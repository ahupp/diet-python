use crate::transformer::{Transformer, walk_stmt};
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr, Stmt, StmtBody};

pub(crate) fn rewrite(body: &mut StmtBody) {
    let mut transformer = TruthyRewriter;
    (&mut transformer).visit_body(body);
}

struct TruthyRewriter;

impl Transformer for &mut TruthyRewriter {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);

        match stmt {
            Stmt::If(ast::StmtIf {
                test,
                elif_else_clauses,
                ..
            }) => {
                wrap_truthy_expr(test);
                for clause in elif_else_clauses {
                    if let Some(test) = &mut clause.test {
                        wrap_truthy_expr(test);
                    }
                }
            }
            Stmt::While(ast::StmtWhile { test, .. }) => {
                wrap_truthy_expr(test);
            }
            _ => {}
        }
    }
}

fn wrap_truthy_expr(expr: &mut Expr) {
    if is_truth_call(expr) {
        return;
    }

    let original = expr.clone();
    *expr = py_expr!(
        "
__dp__.truth({expr:expr})
",
        expr = original,
    );
}

fn is_truth_call(expr: &Expr) -> bool {
    match expr {
        Expr::Call(ast::ExprCall {
            func, arguments, ..
        }) if arguments.args.len() == 1 && arguments.keywords.is_empty() => match func.as_ref() {
            Expr::Attribute(ast::ExprAttribute { value, attr, .. }) if attr.as_str() == "truth" => {
                matches!(
                    value.as_ref(),
                    Expr::Name(ast::ExprName { id, .. }) if id.as_str() == "__dp__"
                ) || matches!(
                    value.as_ref(),
                    Expr::Attribute(ast::ExprAttribute { value, attr, .. })
                        if attr.as_str() == "__dp__"
                            && matches!(value.as_ref(), Expr::Name(ast::ExprName { id, .. }) if id.as_str() == "_dp_global")
                )
            }
            _ => false,
        },
        _ => false,
    }
}
