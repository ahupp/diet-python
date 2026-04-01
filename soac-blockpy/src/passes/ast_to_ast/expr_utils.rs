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

pub(crate) fn make_tuple_splat(elts: Vec<Expr>) -> Expr {
    let mut segments: Vec<Expr> = Vec::new();
    let mut values: Vec<Expr> = Vec::new();

    for elt in elts {
        match elt {
            Expr::Starred(ruff_python_ast::ExprStarred { value, .. }) => {
                if !values.is_empty() {
                    segments.push(make_tuple(std::mem::take(&mut values)));
                }
                segments.push(py_expr!(
                    "__soac__.tuple_from_iter({value:expr})",
                    value = *value
                ));
            }
            other => values.push(other),
        }
    }

    if !values.is_empty() {
        segments.push(make_tuple(values));
    }

    segments
        .into_iter()
        .reduce(|left, right| make_binop("add", left, right))
        .unwrap_or_else(|| make_tuple(Vec::new()))
}

pub(crate) fn make_binop(func_name: &'static str, left: Expr, right: Expr) -> Expr {
    py_expr!(
        "__soac__.{func:id}({left:expr}, {right:expr})",
        left = left,
        right = right,
        func = func_name
    )
}

pub(crate) fn make_unaryop(func_name: &'static str, operand: Expr) -> Expr {
    py_expr!(
        "__soac__.{func:id}({operand:expr})",
        operand = operand,
        func = func_name
    )
}
