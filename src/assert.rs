use ruff_python_ast::visitor::transformer::{walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Stmt};

pub struct AssertRewriter;

impl AssertRewriter {
    pub fn new() -> Self {
        Self
    }
}

impl Transformer for AssertRewriter {
    fn visit_stmt(&self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);

        if let Stmt::Assert(ast::StmtAssert { test, msg, .. }) = stmt {
            let test_expr = *test.clone();
            let new_stmt = if let Some(msg_expr) = msg.clone() {
                crate::py_stmt!(
                    "
if __debug__:
    if not {test:expr}:
        raise AssertionError({msg:expr})
",
                    test = test_expr,
                    msg = *msg_expr
                )
            } else {
                crate::py_stmt!(
                    "
if __debug__:
    if not {test:expr}:
        raise AssertionError
",
                    test = test_expr
                )
            };
            *stmt = new_stmt;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_flatten_eq;
    use ruff_python_ast::visitor::transformer::walk_body;
    use ruff_python_parser::parse_module;

    fn rewrite_assert(source: &str) -> Vec<Stmt> {
        let parsed = parse_module(source).expect("parse error");
        let mut module = parsed.into_syntax();
        let rewriter = AssertRewriter::new();
        walk_body(&rewriter, &mut module.body);
        module.body
    }

    #[test]
    fn rewrites_assert_with_message() {
        let input = "assert a, 'oops'";
        let expected = r#"
if __debug__:
    if not a:
        raise AssertionError('oops')
"#;
        let output = rewrite_assert(input);
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn rewrites_assert_without_message() {
        let input = "assert a";
        let expected = r#"
if __debug__:
    if not a:
        raise AssertionError
"#;
        let output = rewrite_assert(input);
        assert_flatten_eq!(output, expected);
    }
}
