use ruff_python_ast::{self as ast, Stmt};

use crate::{py_expr, py_stmt, transform::driver::{ExprRewriter, Rewrite}};

/// Rewrite decorated functions and classes into explicit decorator applications.
pub fn rewrite(
    decorators: Vec<ast::Decorator>,
    name: &str,
    item: Vec<Stmt>,
    _rewriter: &mut ExprRewriter,
) -> Rewrite {
    if decorators.is_empty() {
        return Rewrite::Walk(item);
    }

    let mut decorated = py_expr!("{name:id}", name = name);
    for decorator in decorators.into_iter().rev() {
        decorated = py_expr!(
            "{decorator:expr}({decorated:expr})",
            decorator = decorator.expression,
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
