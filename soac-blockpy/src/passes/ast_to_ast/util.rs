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

pub(crate) fn is_runtime_attr_lookup_expr(expr: &Expr, attr_name: &str) -> bool {
    matches!(
        expr,
        Expr::Attribute(attr)
            if attr.attr.as_str() == attr_name
                && matches!(
                    attr.value.as_ref(),
                    Expr::Name(module) if module.id.as_str() == "runtime"
                )
    )
}

pub(crate) fn is_dp_helper_lookup_expr(expr: &Expr, helper_name: &str) -> bool {
    matches!(
        expr,
        Expr::Name(name) if name.id.as_str() == format!("__dp_{helper_name}")
    ) || is_runtime_attr_lookup_expr(expr, helper_name)
}

pub(crate) fn strip_synthetic_module_init_qualname(raw: &str) -> String {
    if let Some(rest) = raw.strip_prefix("_dp_module_init.<locals>.") {
        return rest.to_string();
    }
    raw.to_string()
}

pub(crate) fn strip_synthetic_class_namespace_qualname(raw: &str) -> String {
    let mut out = String::new();
    let mut remaining = raw;
    while let Some(pos) = remaining.find("_dp_class_ns_") {
        out.push_str(&remaining[..pos]);
        let rest = &remaining[pos + "_dp_class_ns_".len()..];
        let Some((class_name, tail)) = rest.split_once(".<locals>.") else {
            out.push_str("_dp_class_ns_");
            out.push_str(rest);
            return out;
        };
        out.push_str(class_name);
        if !tail.is_empty() {
            out.push('.');
        }
        remaining = tail;
    }
    if out.is_empty() {
        raw.to_string()
    } else {
        out.push_str(remaining);
        out
    }
}
