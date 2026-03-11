use ruff_python_ast::{self as ast};

use crate::{basic_block::ast_to_ast::ast_rewrite::Rewrite, py_stmt};

pub fn rewrite(ast::StmtAssert { test, msg, .. }: ast::StmtAssert) -> Rewrite {
    Rewrite::Walk(if let Some(msg_expr) = msg {
        py_stmt!(
            "
if __debug__:
    if not {test:expr}:
        raise __dp_AssertionError({msg:expr})
",
            test = test,
            msg = *msg_expr
        )
    } else {
        py_stmt!(
            "
if __debug__:
    if not {test:expr}:
        raise __dp_AssertionError
",
            test = test
        )
    })
}
