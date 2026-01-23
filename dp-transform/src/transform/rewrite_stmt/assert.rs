
use ruff_python_ast::{self as ast};

use crate::{py_stmt, transform::ast_rewrite::Rewrite};

pub fn rewrite(ast::StmtAssert { test, msg, .. }: ast::StmtAssert) -> Rewrite {
    let test_expr = *test;
    if let Some(msg_expr) = msg {
        Rewrite::Visit(py_stmt!(
            "
if __debug__:
    if not {test:expr}:
        raise __dp__.builtins.AssertionError({msg:expr})
",
            test = test_expr,
            msg = *msg_expr
        ))
    } else {
        Rewrite::Visit(py_stmt!(
            "
if __debug__:
    if not {test:expr}:
        raise __dp__.builtins.AssertionError
",
            test = test_expr
        ))
    }
}
