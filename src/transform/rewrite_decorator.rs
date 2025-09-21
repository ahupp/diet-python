use super::{context::Context, expr::Rewrite};
use ruff_python_ast::{self as ast, Stmt};

use crate::{py_expr, py_stmt};

/// Rewrite decorated functions and classes into explicit decorator applications.
pub fn rewrite(
    decorators: Vec<ast::Decorator>,
    name: &str,
    mut item: Vec<Stmt>,
    _ctx: &Context,
) -> Rewrite {
    if decorators.is_empty() {
        return Rewrite::Walk(item);
    }

    let mut assignments: Vec<Stmt> = Vec::new();
    let mut decorator_names: Vec<String> = Vec::new();

    for (index, decorator) in decorators.into_iter().enumerate() {
        let temp_name = format!("_dp_decorator_{name}_{index}");
        assignments.extend(py_stmt!(
            "{temp_name:id} = {decorator:expr}",
            temp_name = temp_name.as_str(),
            decorator = decorator.expression
        ));
        decorator_names.push(temp_name);
    }

    let mut decorated = py_expr!("{name:id}", name = name);
    for decorator_name in decorator_names.iter().rev() {
        decorated = py_expr!(
            "{decorator:id}({decorated:expr})",
            decorator = decorator_name.as_str(),
            decorated = decorated
        );
    }

    let mut result = assignments;
    result.append(&mut item);
    result.extend(py_stmt!(
        "{name:id} = {decorated:expr}",
        name = name,
        decorated = decorated
    ));

    Rewrite::Visit(result)
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_decorator.txt");
}
