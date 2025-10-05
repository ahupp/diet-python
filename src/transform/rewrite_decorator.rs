use super::driver::Rewrite;
use ruff_python_ast::{self as ast, Stmt};

use crate::{py_expr, py_stmt, transform::driver::ExprRewriter};

/// Rewrite decorated functions and classes into explicit decorator applications.
pub fn rewrite(
    decorators: Vec<ast::Decorator>,
    name: &str,
    item: Vec<Stmt>,
    rewriter: &mut ExprRewriter,
) -> Rewrite {
    if decorators.is_empty() {
        return Rewrite::Walk(item);
    }

    let to_apply = decorators
        .into_iter()
        .map(|decorator| rewriter.maybe_placeholder(decorator.expression))
        .collect::<Vec<_>>();

    let mut decorated = py_expr!("{name:id}", name = name);
    for decorator in to_apply.into_iter().rev() {
        decorated = py_expr!(
            "{decorator:expr}({decorated:expr})",
            decorator = decorator,
            decorated = decorated
        );
    }

    Rewrite::Visit(py_stmt!(
        r#"
{item:stmt}
{name:id} = {decorated:expr}
"#,
        name = name,
        item = item,
        decorated = decorated
    ))
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_decorator.txt");
}
