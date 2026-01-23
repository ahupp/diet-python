use ruff_python_ast::{self as ast, Stmt};

use crate::{py_expr, py_stmt, transform::{ast_rewrite::Rewrite, context::Context}};

/// Rewrite decorated functions and classes into explicit decorator applications.
pub fn rewrite(
    context: &Context,
    decorators: Vec<ast::Decorator>,
    name: &str,
    item: Vec<Stmt>,
) -> Rewrite {
    if decorators.is_empty() {
        return Rewrite::Walk(item);
    }

    let mut prefix = Vec::new();
    let mut decorator_names = Vec::with_capacity(decorators.len());
    for decorator in decorators.into_iter() {
        let temp = context.fresh("decorator");
        prefix.extend(py_stmt!(
            "{temp:id} = {decorator:expr}",
            temp = temp.as_str(),
            decorator = decorator.expression
        ));
        decorator_names.push(temp);
    }

    let mut decorated = py_expr!("{name:id}", name = name);
    for decorator in decorator_names.iter().rev() {
        decorated = py_expr!(
            "{decorator:id}({decorated:expr})",
            decorator = decorator.as_str(),
            decorated = decorated
        );
    }

    let mut stmts = Vec::new();
    stmts.extend(prefix);
    stmts.extend(item);
    stmts.extend(py_stmt!(
        "{name:id} = {decorated:expr}",
        name = name,
        decorated = decorated
    ));
    Rewrite::Walk(stmts)
}
