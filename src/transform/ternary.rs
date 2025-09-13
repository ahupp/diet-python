use ruff_python_ast::visitor::transformer::{walk_expr, Transformer};
use ruff_python_ast::{self as ast, Expr};

pub struct TernaryRewriter;

impl TernaryRewriter {
    pub fn new() -> Self {
        Self
    }
}

impl Transformer for TernaryRewriter {
    fn visit_expr(&self, expr: &mut Expr) {
        walk_expr(self, expr);
        if let Expr::If(ast::ExprIf {
            test, body, orelse, ..
        }) = expr
        {
            let test_expr = *test.clone();
            let body_expr = *body.clone();
            let orelse_expr = *orelse.clone();
            *expr = crate::py_expr!(
                "
__dp__.if_expr({cond:expr}, lambda: {body:expr}, lambda: {orelse:expr})
",
                cond = test_expr,
                body = body_expr,
                orelse = orelse_expr,
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
        let rewriter = TernaryRewriter::new();
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
    fn rewrites_if_expr() {
        let cases = [
            (
                r#"
a if b else c
"#,
                r#"
__dp__.if_expr(b, lambda: a, lambda: c)
"#,
            ),
            (
                r#"
(a + 1) if f() else (b + 2)
"#,
                r#"
__dp__.if_expr(f(), lambda: a + 1, lambda: b + 2)
"#,
            ),
        ];
        for (input, expected) in cases {
            let output = rewrite(input);
            assert_eq!(output.trim(), expected.trim());
        }
    }
}
