use ruff_python_ast::{self as ast, Expr};

pub(crate) fn is_noarg_call(name: &str, expr: &Expr) -> bool {
    let Expr::Call(ast::ExprCall {
        func, arguments, ..
    }) = expr
    else {
        return false;
    };
    let Expr::Name(ast::ExprName { id, .. }) = func.as_ref() else {
        return false;
    };
    id.as_str() == name && arguments.args.is_empty() && arguments.keywords.is_empty()
}
