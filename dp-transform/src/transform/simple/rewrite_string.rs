use crate::{py_expr, template::make_tuple};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::Ranged;

use crate::transform::context::Context;

fn join_parts(parts: Vec<Expr>, force_join: bool) -> Expr {
    match parts.len() {
        0 => py_expr!("\"\""),
        1 if !force_join => parts.into_iter().next().unwrap(),
        _ => {
            let tuple = make_tuple(parts);
            py_expr!("\"\".join({tuple:expr})", tuple = tuple)
        }
    }
}

fn strip_debug_comment(trailing: &str) -> String {
    let mut output = String::with_capacity(trailing.len());
    let mut in_comment = false;
    for ch in trailing.chars() {
        if in_comment {
            if ch == '\n' {
                output.push(ch);
                in_comment = false;
            }
            continue;
        }
        if ch == '#' {
            in_comment = true;
            continue;
        }
        output.push(ch);
    }
    output
}

fn rewrite_interpolation(interp: &ast::InterpolatedElement, ctx: &Context) -> Vec<Expr> {
    let mut parts = Vec::new();

    let mut value = (*interp.expression).clone();
    let conversion = if let Some(debug) = &interp.debug_text {
        let has_format_spec = interp.format_spec.is_some();
        let trailing_has_format = debug.trailing.contains(':');
        if matches!(interp.conversion, ast::ConversionFlag::None) && !has_format_spec {
            ast::ConversionFlag::Repr
        } else if matches!(interp.conversion, ast::ConversionFlag::Repr)
            && has_format_spec
            && trailing_has_format
        {
            ast::ConversionFlag::None
        } else {
            interp.conversion
        }
    } else {
        interp.conversion
    };
    value = match conversion {
        ast::ConversionFlag::Ascii => py_expr!("__dp__.builtins.ascii({value:expr})", value = value),
        ast::ConversionFlag::Repr => py_expr!("__dp__.builtins.repr({value:expr})", value = value),
        ast::ConversionFlag::Str => py_expr!("__dp__.builtins.str({value:expr})", value = value),
        ast::ConversionFlag::None => value,
    };

    if let Some(debug) = &interp.debug_text {
        let expr_range = interp.expression.range();
        let expr_text = ctx.source_slice(expr_range).unwrap_or("");
        let trailing = strip_debug_comment(debug.trailing.as_str());
        let debug_text = format!("{}{}{}", debug.leading, expr_text, trailing);
        if !debug_text.is_empty() {
            parts.push(py_expr!("{literal:literal}", literal = debug_text.as_str()));
        }
    }

    let formatted = if let Some(format_spec) = &interp.format_spec {
        let (parts, _) = rewrite_elements(&format_spec.elements, ctx);
        let spec = if parts.is_empty() {
            py_expr!("\"\"")
        } else {
            join_parts(parts, false)
        };
        py_expr!(
            "__dp__.builtins.format({value:expr}, {format_spec:expr})",
            value = value,
            format_spec = spec
        )
    } else {
        py_expr!("__dp__.builtins.format({value:expr})", value = value)
    };

    parts.push(formatted);
    parts
}

fn rewrite_elements(
    elements: &ast::InterpolatedStringElements,
    ctx: &Context,
) -> (Vec<Expr>, bool) {
    let mut parts = Vec::new();
    let mut has_interpolation = false;
    for element in elements.iter() {
        match element {
            ast::InterpolatedStringElement::Literal(lit) => {
                parts.push(py_expr!("{literal:literal}", literal = lit.value.as_ref()));
            }
            ast::InterpolatedStringElement::Interpolation(interp) => {
                parts.extend(rewrite_interpolation(interp, ctx));
                has_interpolation = true;
            }
        }
    }
    (parts, has_interpolation)
}

fn rewrite_tstring_interpolation(interp: &ast::InterpolatedElement, ctx: &Context) -> Expr {
    let value = (*interp.expression).clone();
    let expr_text = ctx
        .source_slice(interp.expression.range())
        .unwrap_or("")
        .to_string();
    let conversion_expr = match interp.conversion {
        ast::ConversionFlag::None => py_expr!("None"),
        ast::ConversionFlag::Repr => py_expr!("{literal:literal}", literal = "r"),
        ast::ConversionFlag::Str => py_expr!("{literal:literal}", literal = "s"),
        ast::ConversionFlag::Ascii => py_expr!("{literal:literal}", literal = "a"),
    };
    let format_spec = if let Some(format_spec) = &interp.format_spec {
        let (parts, _) = rewrite_elements(&format_spec.elements, ctx);
        if parts.is_empty() {
            py_expr!("{literal:literal}", literal = "")
        } else {
            join_parts(parts, false)
        }
    } else {
        py_expr!("{literal:literal}", literal = "")
    };
    py_expr!(
        "__dp__.interpolation({value:expr}, {expr_text:literal}, {conversion:expr}, {format_spec:expr})",
        value = value,
        expr_text = expr_text.as_str(),
        conversion = conversion_expr,
        format_spec = format_spec,
    )
}

pub fn rewrite_fstring(expr: ast::ExprFString, ctx: &Context) -> Expr {
    let mut parts = Vec::new();
    let mut has_interpolation = false;
    for part in expr.value.iter() {
        match part {
            ast::FStringPart::Literal(lit) => {
                parts.push(py_expr!("{literal:literal}", literal = lit.value.as_ref()));
            }
            ast::FStringPart::FString(f) => {
                let (elements, has_interp) = rewrite_elements(&f.elements, ctx);
                parts.extend(elements);
                has_interpolation |= has_interp;
            }
        }
    }
    join_parts(parts, !has_interpolation)
}

pub fn rewrite_tstring(expr: ast::ExprTString, _ctx: &Context) -> Expr {
    let mut parts = Vec::new();
    for t in expr.value.iter() {
        for element in t.elements.iter() {
            match element {
                ast::InterpolatedStringElement::Literal(lit) => {
                    parts.push(py_expr!("{literal:literal}", literal = lit.value.as_ref()));
                }
                ast::InterpolatedStringElement::Interpolation(interp) => {
                    parts.push(rewrite_tstring_interpolation(interp, _ctx));
                }
            }
        }
    }
    let tuple = make_tuple(parts);
    py_expr!("__dp__.template(*{parts:expr})", parts = tuple)
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_string.txt");
}
