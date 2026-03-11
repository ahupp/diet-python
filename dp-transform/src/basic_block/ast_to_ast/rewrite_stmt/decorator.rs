use ruff_python_ast::{self as ast, Expr};

use crate::py_expr;

/// Rewrite decorated functions and classes into explicit decorator applications.
pub fn rewrite(decorators: Vec<ast::Decorator>, mut to_decorate: Expr) -> Expr {
    for decorator in decorators.into_iter().rev() {
        to_decorate = py_expr!(
            "({decorator:expr})({to_decorate:expr})",
            decorator = decorator.expression,
            to_decorate = to_decorate
        );
    }

    to_decorate
}
