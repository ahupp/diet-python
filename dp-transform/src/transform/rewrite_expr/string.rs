use crate::{py_expr, transform::rewrite_expr::make_tuple};
use ruff_python_ast::str_prefix::StringLiteralPrefix;
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

fn rewrite_interpolation(
    interp: &ast::InterpolatedElement,
    ctx: &Context,
    is_raw: bool,
) -> Vec<Expr> {
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
        ast::ConversionFlag::Ascii => {
            py_expr!("__dp__.builtins.ascii({value:expr})", value = value)
        }
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
        let (parts, _) = rewrite_elements(&format_spec.elements, ctx, is_raw);
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
    is_raw: bool,
) -> (Vec<Expr>, bool) {
    let mut parts = Vec::new();
    let mut has_interpolation = false;
    for element in elements.iter() {
        match element {
            ast::InterpolatedStringElement::Literal(lit) => {
                parts.push(rewrite_fstring_literal(lit, ctx, is_raw));
            }
            ast::InterpolatedStringElement::Interpolation(interp) => {
                parts.extend(rewrite_interpolation(interp, ctx, is_raw));
                has_interpolation = true;
            }
        }
    }
    (parts, has_interpolation)
}

fn rewrite_tstring_interpolation(interp: &ast::InterpolatedElement, ctx: &Context) -> Expr {
    ctx.require_templatelib_import();
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
        let (parts, _) = rewrite_elements(&format_spec.elements, ctx, false);
        if parts.is_empty() {
            py_expr!("{literal:literal}", literal = "")
        } else {
            join_parts(parts, false)
        }
    } else {
        py_expr!("{literal:literal}", literal = "")
    };
    py_expr!(
        "_dp_templatelib.Interpolation({value:expr}, {expr_text:literal}, {conversion:expr}, {format_spec:expr})",
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
                parts.push(rewrite_string_literal(lit, ctx));
            }
            ast::FStringPart::FString(f) => {
                let (elements, has_interp) =
                    rewrite_elements(&f.elements, ctx, f.flags.prefix().is_raw());
                parts.extend(elements);
                has_interpolation |= has_interp;
            }
        }
    }
    join_parts(parts, !has_interpolation)
}

pub fn rewrite_tstring(expr: ast::ExprTString, _ctx: &Context) -> Expr {
    _ctx.require_templatelib_import();
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
    py_expr!("_dp_templatelib.Template(*{parts:expr})", parts = tuple)
}

fn rewrite_string_literal(lit: &ast::StringLiteral, ctx: &Context) -> Expr {
    if matches!(lit.flags.prefix(), StringLiteralPrefix::Raw { .. }) {
        return py_expr!("{literal:literal}", literal = lit.value.as_ref());
    }
    if let Some(content) = ctx.source_slice(lit.content_range()) {
        if has_surrogate_escape(content) {
            if let Some(src) = ctx.source_slice(lit.range) {
                return py_expr!(
                    "__dp__.decode_surrogate_literal({literal:literal})",
                    literal = src
                );
            }
        }
    }
    py_expr!("{literal:literal}", literal = lit.value.as_ref())
}

fn rewrite_fstring_literal(
    lit: &ast::InterpolatedStringLiteralElement,
    ctx: &Context,
    is_raw: bool,
) -> Expr {
    if !is_raw {
        if let Some(src) = ctx.source_slice(lit.range) {
            if has_surrogate_escape(src) {
                let literal_src = quote_fstring_literal(src);
                return py_expr!(
                    "__dp__.decode_surrogate_literal({literal:literal})",
                    literal = literal_src.as_str()
                );
            }
        }
    }
    py_expr!("{literal:literal}", literal = lit.value.as_ref())
}

fn quote_fstring_literal(raw: &str) -> String {
    let quote = if raw.contains('\'') && !raw.contains('"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::with_capacity(raw.len() + 2);
    out.push(quote);
    let mut backslashes = 0usize;
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '{' {
            if matches!(chars.peek(), Some('{')) {
                chars.next();
                out.push('{');
                backslashes = 0;
                continue;
            }
        } else if ch == '}' {
            if matches!(chars.peek(), Some('}')) {
                chars.next();
                out.push('}');
                backslashes = 0;
                continue;
            }
        }
        if ch == '\\' {
            backslashes += 1;
            out.push('\\');
            continue;
        }
        if ch == '\n' {
            out.push('\\');
            out.push('n');
            backslashes = 0;
            continue;
        }
        if ch == '\r' {
            out.push('\\');
            out.push('r');
            backslashes = 0;
            continue;
        }
        if ch == quote {
            if backslashes % 2 == 0 {
                out.push('\\');
            }
            out.push(ch);
        } else {
            out.push(ch);
        }
        backslashes = 0;
    }
    out.push(quote);
    out
}

fn has_surrogate_escape(content: &str) -> bool {
    let bytes = content.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'\\' {
            i += 1;
            continue;
        }
        if i + 1 >= bytes.len() {
            break;
        }
        match bytes[i + 1] {
            b'u' => {
                if i + 5 < bytes.len() {
                    if let Some(value) = parse_hex(&bytes[i + 2..i + 6]) {
                        if (0xD800..=0xDFFF).contains(&value) {
                            return true;
                        }
                    }
                    i += 6;
                    continue;
                }
                i += 2;
            }
            b'U' => {
                if i + 9 < bytes.len() {
                    if let Some(value) = parse_hex(&bytes[i + 2..i + 10]) {
                        if (0xD800..=0xDFFF).contains(&value) {
                            return true;
                        }
                    }
                    i += 10;
                    continue;
                }
                i += 2;
            }
            _ => {
                i += 2;
            }
        }
    }
    false
}

fn parse_hex(bytes: &[u8]) -> Option<u32> {
    let mut value: u32 = 0;
    for &b in bytes {
        value <<= 4;
        value |= match b {
            b'0'..=b'9' => (b - b'0') as u32,
            b'a'..=b'f' => (b - b'a' + 10) as u32,
            b'A'..=b'F' => (b - b'A' + 10) as u32,
            _ => return None,
        };
    }
    Some(value)
}
