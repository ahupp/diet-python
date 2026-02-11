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

pub(crate) fn strip_synthetic_module_init_qualname(raw: &str) -> String {
    if let Some(rest) = raw.strip_prefix("_dp_module_init.<locals>.") {
        return rest.to_string();
    }
    if raw.starts_with("_dp_fn__dp_module_init_") {
        if let Some((_, tail)) = raw.split_once(".<locals>.") {
            return tail.to_string();
        }
    }
    raw.to_string()
}
