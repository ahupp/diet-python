use super::context::Context;
use ruff_python_ast::{self as ast, Stmt};

use crate::{py_expr, py_stmt};

/// Rewrite decorated functions and classes into explicit decorator applications.
pub fn rewrite(
    decorators: Vec<ast::Decorator>,
    name: &str,
    item: Vec<Stmt>,
    _ctx: &Context,
) -> Vec<Stmt> {
    let decorator_expr =
        decorators
            .into_iter()
            .rev()
            .fold(py_expr!("_dp_the_func"), |acc, decorator| {
                py_expr!(
                    "{decorator:expr}({acc:expr})",
                    decorator = decorator.expression,
                    acc = acc
                )
            });

    py_stmt!(
        r#"
def _dp_decorator_{name:id}(_dp_the_func):
    return {decorator_expr:expr}
{item:stmt}
{name:id} = _dp_decorator_{name:id}({name:id})"#,
        decorator_expr = decorator_expr,
        item = item,
        name = name,
    )
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_decorator.txt");
}
