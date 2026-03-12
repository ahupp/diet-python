use crate::py_expr;
use ruff_python_ast::Expr;
use ruff_python_parser::parse_expression;

pub(crate) fn make_dp_tuple(items: Vec<Expr>) -> Expr {
    let Expr::Call(mut call) = py_expr!("__dp_tuple()") else {
        panic!("expected call expression for __dp_tuple");
    };
    call.arguments.args = items.into();
    Expr::Call(call)
}

pub(crate) fn name_expr(name: &str) -> Option<Expr> {
    parse_expression(name)
        .ok()
        .map(|expr| *expr.into_syntax().body)
}
