use ruff_python_ast::{self as ast, Stmt};

use crate::py_stmt;

pub fn rewrite(ast::StmtAssert { test, msg, .. }: ast::StmtAssert) -> Stmt {
    let test_expr = *test;
    if let Some(msg_expr) = msg {
        py_stmt!(
            "
if __debug__:
    if not {test:expr}:
        raise AssertionError({msg:expr})
",
            test = test_expr,
            msg = *msg_expr
        )
    } else {
        py_stmt!(
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
    use crate::test_util::assert_transform_eq;

    #[test]
    fn rewrites_assert_with_message() {
        let input = "assert a, 'oops'";
        let expected = r#"
if __debug__:
    if __dp__.not_(a):
        raise AssertionError('oops')
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_assert_without_message() {
        let input = "assert a";
        let expected = r#"
if __debug__:
    if __dp__.not_(a):
        raise AssertionError
"#;
        assert_transform_eq(input, expected);
    }
}
