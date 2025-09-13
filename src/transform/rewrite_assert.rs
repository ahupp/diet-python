use ruff_python_ast::{self as ast, Stmt};

pub fn rewrite(ast::StmtAssert { test, msg, .. }: ast::StmtAssert) -> Stmt {
    let test_expr = *test;
    if let Some(msg_expr) = msg {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_flatten_eq;
    use crate::transform::expr::ExprRewriter;
    use ruff_python_ast::visitor::transformer::walk_body;
    use ruff_python_parser::parse_module;

    fn rewrite_assert(source: &str) -> Vec<Stmt> {
        let parsed = parse_module(source).expect("parse error");
        let mut module = parsed.into_syntax();
        let rewriter = ExprRewriter::new();
        walk_body(&rewriter, &mut module.body);
        module.body
    }

    #[test]
    fn rewrites_assert_with_message() {
        let input = "assert a, 'oops'";
        let expected = r#"
if __debug__:
    if _dp_not_(a):
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
    if _dp_not_(a):
        raise AssertionError
"#;
        let output = rewrite_assert(input);
        assert_flatten_eq!(output, expected);
    }
}
