use crate::py_expr;
use ruff_python_ast::Expr;

pub(crate) fn make_tuple(items: Vec<Expr>) -> Expr {
    let Expr::Call(mut call) = py_expr!("__soac__.tuple_values()") else {
        panic!("expected call expression for __soac__.tuple_values");
    };
    call.arguments.args = items.into();
    Expr::Call(call)
}

pub(crate) fn make_dp_tuple(items: Vec<Expr>) -> Expr {
    make_tuple(items)
}
