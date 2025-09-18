use super::context::Context;
use ruff_python_ast::{self as ast, Stmt};

use crate::{py_expr, py_stmt};

/// Rewrite decorated functions and classes into explicit decorator applications.
pub fn rewrite(
    decorators: Vec<ast::Decorator>,
    name: &str,
    item: Stmt,
    base: Option<&str>,
    ctx: &Context,
) -> Stmt {
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

    let base_or_name = base.unwrap_or(name);

    let dec_apply_fn = base
        .map(|_| format!("_dp_class_decorators_{}", name))
        .unwrap_or_else(|| ctx.fresh("dec_apply"));

    py_stmt!(
        r#"
def {dec_apply_fn:id}(_dp_the_func):
    return {decorator_expr:expr}
{item:stmt}
{name:id} = {dec_apply_fn:id}({base_or_name:id})"#,
        dec_apply_fn = dec_apply_fn.as_str(),
        decorator_expr = decorator_expr,
        item = item,
        name = name,
        base_or_name = base_or_name,
    )
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_decorator.txt");
}
