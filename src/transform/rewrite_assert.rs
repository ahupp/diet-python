use ruff_python_ast::{self as ast, Stmt};

use crate::py_stmt;

pub fn rewrite(ast::StmtAssert { test, msg, .. }: ast::StmtAssert) -> Vec<Stmt> {
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
    crate::transform_fixture_test!("tests_rewrite_assert.txt");
}
