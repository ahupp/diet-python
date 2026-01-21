use super::driver::Rewrite;
use ruff_python_ast::{self as ast, Expr, Stmt};

use crate::{py_expr, py_stmt, transform::driver::ExprRewriter};

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

    let decorated = apply(decorators, py_expr!("{name:id}", name = name));

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

pub fn apply(decorators: Vec<ast::Decorator>, base: Expr) -> Expr {
    let mut decorated = base;
    for decorator in decorators.into_iter().rev() {
        decorated = py_expr!(
            "{decorator:expr}({decorated:expr})",
            decorator = decorator.expression,
            decorated = decorated
        );
    }
    decorated
}

pub fn apply_exprs(decorators: Vec<Expr>, base: Expr) -> Expr {
    let mut decorated = base;
    for decorator in decorators.into_iter().rev() {
        decorated = py_expr!(
            "{decorator:expr}({decorated:expr})",
            decorator = decorator,
            decorated = decorated
        );
    }
    decorated
}
