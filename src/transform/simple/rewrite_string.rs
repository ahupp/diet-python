use crate::{py_expr, template::make_tuple};
use ruff_python_ast::{self as ast, Expr};

fn join_parts(parts: Vec<Expr>) -> Expr {
    match parts.len() {
        0 => py_expr!("\"\""),
        1 => parts.into_iter().next().unwrap(),
        _ => {
            let tuple = make_tuple(parts);
            py_expr!("\"\".join({tuple:expr})", tuple = tuple)
        }
    }
}

fn rewrite_interpolation(interp: &ast::InterpolatedElement) -> Vec<Expr> {
    let mut parts = Vec::new();

    if let Some(debug) = &interp.debug_text {
        if !debug.leading.is_empty() {
            parts.push(py_expr!(
                "{literal:literal}",
                literal = debug.leading.as_str()
            ));
        }
    }

    let mut value = (*interp.expression).clone();
    value = match interp.conversion {
        ast::ConversionFlag::Ascii => py_expr!("ascii({value:expr})", value = value),
        ast::ConversionFlag::Repr => py_expr!("repr({value:expr})", value = value),
        ast::ConversionFlag::Str => py_expr!("str({value:expr})", value = value),
        ast::ConversionFlag::None => value,
    };

    let formatted = if let Some(format_spec) = &interp.format_spec {
        let parts = rewrite_elements(&format_spec.elements);
        let spec = if parts.is_empty() {
            py_expr!("\"\"")
        } else {
            join_parts(parts)
        };
        py_expr!(
            "format({value:expr}, {format_spec:expr})",
            value = value,
            format_spec = spec
        )
    } else {
        py_expr!("format({value:expr})", value = value)
    };

    parts.push(formatted);
    if let Some(debug) = &interp.debug_text {
        if !debug.trailing.is_empty() {
            parts.push(py_expr!(
                "{literal:literal}",
                literal = debug.trailing.as_str()
            ));
        }
    }
    parts
}

fn rewrite_elements(elements: &ast::InterpolatedStringElements) -> Vec<Expr> {
    let mut parts = Vec::new();
    for element in elements.iter() {
        match element {
            ast::InterpolatedStringElement::Literal(lit) => {
                parts.push(py_expr!("{literal:literal}", literal = lit.value.as_ref()));
            }
            ast::InterpolatedStringElement::Interpolation(interp) => {
                parts.extend(rewrite_interpolation(interp));
            }
        }
    }
    parts
}

pub fn rewrite_fstring(expr: ast::ExprFString) -> Expr {
    let mut parts = Vec::new();
    for part in expr.value.iter() {
        match part {
            ast::FStringPart::Literal(lit) => {
                parts.push(py_expr!("{literal:literal}", literal = lit.value.as_ref()));
            }
            ast::FStringPart::FString(f) => {
                parts.extend(rewrite_elements(&f.elements));
            }
        }
    }
    join_parts(parts)
}

pub fn rewrite_tstring(expr: ast::ExprTString) -> Expr {
    let mut parts = Vec::new();
    for t in expr.value.iter() {
        parts.extend(rewrite_elements(&t.elements));
    }
    join_parts(parts)
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_string.txt");
}
