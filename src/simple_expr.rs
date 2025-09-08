use ruff_python_ast as ast;
use ruff_python_ast::visitor::transformer::{walk_expr, Transformer};
use ruff_python_ast::Expr;
use ruff_text_size::TextRange;

/// Rewrites slice expressions ``a:b:c`` into ``slice(a, b, c)`` calls,
/// complex number literals into ``complex(real, imag)`` calls, ellipsis
/// expressions into references to the ``Ellipsis`` singleton, and attribute
/// access into ``getattr(obj, "attr")`` calls.
pub struct SimpleExprTransformer;

impl SimpleExprTransformer {
    pub fn new() -> Self {
        Self
    }
}

impl Transformer for SimpleExprTransformer {
    fn visit_expr(&self, expr: &mut Expr) {
        walk_expr(self, expr);
        match expr {
            Expr::Slice(ast::ExprSlice { lower, upper, step, .. }) => {
                let lower_expr = lower
                    .as_ref()
                    .map(|expr| *expr.clone())
                    .unwrap_or_else(|| crate::py_expr!("None"));
                let upper_expr = upper
                    .as_ref()
                    .map(|expr| *expr.clone())
                    .unwrap_or_else(|| crate::py_expr!("None"));
                let step_expr = step
                    .as_ref()
                    .map(|expr| *expr.clone())
                    .unwrap_or_else(|| crate::py_expr!("None"));

                *expr = crate::py_expr!(
                    "slice({lower:expr}, {upper:expr}, {step:expr})",
                    lower = lower_expr,
                    upper = upper_expr,
                    step = step_expr,
                );
            }
            Expr::EllipsisLiteral(_) => {
                *expr = crate::py_expr!("Ellipsis");
            }
            Expr::NumberLiteral(ast::ExprNumberLiteral {
                value: ast::Number::Complex { real, imag },
                ..
            }) => {
                let real_expr = Expr::NumberLiteral(ast::ExprNumberLiteral {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: TextRange::default(),
                    value: ast::Number::Float(*real),
                });
                let imag_expr = Expr::NumberLiteral(ast::ExprNumberLiteral {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: TextRange::default(),
                    value: ast::Number::Float(*imag),
                });
                *expr = crate::py_expr!(
                    "complex({real:expr}, {imag:expr})",
                    real = real_expr,
                    imag = imag_expr,
                );
            }
            Expr::Attribute(ast::ExprAttribute { value, attr, ctx, .. }) => {
                if matches!(ctx, ast::ExprContext::Load) {
                    let value_expr = *value.clone();
                    let attr_expr = crate::py_expr!("\"{name:id}\"", name = attr.id.as_str());
                    *expr = crate::py_expr!(
                        "getattr({value:expr}, {attr:expr})",
                        value = value_expr,
                        attr = attr_expr,
                    );
                    // TODO: figure out the bootstrapping problem.
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruff_python_ast::visitor::transformer::walk_body;
    use ruff_python_codegen::{Generator, Stylist};
    use ruff_python_parser::parse_module;

    fn rewrite(source: &str) -> String {
        let parsed = parse_module(source).expect("parse error");
        let tokens = parsed.tokens().clone();
        let mut module = parsed.into_syntax();

        let rewriter = SimpleExprTransformer::new();
        walk_body(&rewriter, &mut module.body);

        let stylist = Stylist::from_tokens(&tokens, source);
        let mut output = String::new();
        for stmt in &module.body {
            let snippet = Generator::from(&stylist).stmt(stmt);
            output.push_str(&snippet);
            output.push_str(stylist.line_ending().as_str());
        }
        output
    }

    #[test]
    fn rewrites_slices() {
        let cases = [
            ("a[1:2:3]", "a[slice(1, 2, 3)]"),
            ("a[1:2]", "a[slice(1, 2, None)]"),
            ("a[:2]", "a[slice(None, 2, None)]"),
            ("a[::2]", "a[slice(None, None, 2)]"),
            ("a[:]", "a[slice(None, None, None)]"),
        ];

        for (input, expected) in cases {
            let output = rewrite(input);
            assert_eq!(output.trim_end(), expected);
        }
    }

    #[test]
    fn rewrites_complex_literals() {
        let cases = [
            ("a = 1j", "a = complex(0.0, 1.0)"),
            ("a = 1 + 2j", "a = 1 + complex(0.0, 2.0)"),
        ];

        for (input, expected) in cases {
            let output = rewrite(input);
            assert_eq!(output.trim_end(), expected);
        }
    }

    #[test]
    fn rewrites_ellipsis() {
        let cases = [
            ("a = ...", "a = Ellipsis"),
            ("...", "Ellipsis"),
        ];

        for (input, expected) in cases {
            let output = rewrite(input);
            assert_eq!(output.trim_end(), expected);
        }
    }

    #[test]
    fn rewrites_attribute_access() {
        let cases = [
            ("obj.attr", "getattr(obj, \"attr\")"),
            (
                "foo.bar.baz",
                "getattr(getattr(foo, \"bar\"), \"baz\")",
            ),
        ];

        for (input, expected) in cases {
            let output = rewrite(input);
            assert_eq!(output.trim_end(), expected);
        }
    }
}
