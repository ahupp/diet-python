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

fn expr_static_str(expr: &Expr) -> Option<String> {
    match expr {
        Expr::StringLiteral(value) => Some(value.value.to_str().to_string()),
        Expr::Call(call)
            if call.arguments.keywords.is_empty()
                && call.arguments.args.len() == 1
                && matches!(
                    call.func.as_ref(),
                    Expr::Name(name) if name.id.as_str() == "__dp_decode_literal_bytes"
                ) =>
        {
            match &call.arguments.args[0] {
                Expr::BytesLiteral(bytes) => {
                    let value: std::borrow::Cow<[u8]> = (&bytes.value).into();
                    String::from_utf8(value.into_owned()).ok()
                }
                _ => None,
            }
        }
        _ => None,
    }
}

pub(crate) fn is_runtime_attr_lookup_expr(expr: &Expr, attr_name: &str) -> bool {
    if let Expr::Attribute(attr) = expr {
        return attr.attr.as_str() == attr_name
            && matches!(
                attr.value.as_ref(),
                Expr::Name(module) if module.id.as_str() == "runtime"
            );
    }
    let Expr::Call(call) = expr else {
        return false;
    };
    if !call.arguments.keywords.is_empty() || call.arguments.args.len() != 2 {
        return false;
    }
    if !matches!(
        call.func.as_ref(),
        Expr::Name(name) if name.id.as_str() == "__dp_getattr"
    ) {
        return false;
    }
    let base_matches = matches!(
        &call.arguments.args[0],
        Expr::Name(base) if base.id.as_str() == "runtime"
    );
    base_matches && expr_static_str(&call.arguments.args[1]).as_deref() == Some(attr_name)
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
