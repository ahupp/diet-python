use ruff_python_ast::{self as ast, Expr};

use crate::py_expr;

pub fn collect_exprs(decorators: &[ast::Decorator]) -> Vec<Expr> {
    decorators
        .iter()
        .map(|decorator| decorator.expression.clone())
        .collect()
}

pub fn into_exprs(decorators: Vec<ast::Decorator>) -> Vec<Expr> {
    decorators
        .into_iter()
        .map(|decorator| decorator.expression)
        .collect()
}

pub fn rewrite_exprs(decorators: Vec<Expr>, mut to_decorate: Expr) -> Expr {
    for decorator in decorators.into_iter().rev() {
        to_decorate = py_expr!(
            "({decorator:expr})({to_decorate:expr})",
            decorator = decorator,
            to_decorate = to_decorate
        );
    }

    to_decorate
}

/// Rewrite decorated functions and classes into explicit decorator applications.
pub fn rewrite(decorators: Vec<ast::Decorator>, to_decorate: Expr) -> Expr {
    rewrite_exprs(into_exprs(decorators), to_decorate)
}

#[cfg(test)]
mod tests {
    use super::rewrite_exprs;

    #[test]
    fn rewrite_exprs_applies_decorators_inside_out() {
        let decorated = rewrite_exprs(
            vec![crate::py_expr!("d1"), crate::py_expr!("d2")],
            crate::py_expr!("f"),
        );
        assert_eq!(crate::ruff_ast_to_string(&decorated).trim(), "d1(d2(f))");
    }
}
