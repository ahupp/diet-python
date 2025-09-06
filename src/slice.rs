use ruff_python_ast as ast;
use ruff_python_ast::visitor::transformer::{walk_expr, Transformer};
use ruff_python_ast::Expr;

/// Rewrites slice expressions ``a:b:c`` into ``slice(a, b, c)`` calls.
pub struct SliceRewriter;

impl SliceRewriter {
    pub fn new() -> Self {
        Self
    }
}

impl Transformer for SliceRewriter {
    fn visit_expr(&self, expr: &mut Expr) {
        walk_expr(self, expr);
        if let Expr::Slice(ast::ExprSlice {
            lower, upper, step, ..
        }) = expr
        {
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

        let rewriter = SliceRewriter::new();
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
}
