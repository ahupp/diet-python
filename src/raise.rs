use ruff_python_ast::visitor::transformer::{walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Stmt};

pub struct RaiseRewriter;

impl RaiseRewriter {
    pub fn new() -> Self {
        Self
    }
}

impl Transformer for RaiseRewriter {
    fn visit_stmt(&self, stmt: &mut Stmt) {
        walk_stmt(self, stmt);

        if let Stmt::Raise(ast::StmtRaise {
            exc: Some(exc),
            cause: Some(cause),
            ..
        }) = stmt
        {
            let exc_expr = *exc.clone();
            let cause_expr = *cause.clone();
            *stmt = crate::py_stmt!(
                "dp_intrinsics.raise_from({exc:expr}, {cause:expr})",
                exc = exc_expr,
                cause = cause_expr,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_flatten_eq;
    use ruff_python_ast::visitor::transformer::walk_body;
    use ruff_python_ast::Stmt;
    use ruff_python_parser::parse_module;

    fn rewrite_raise(source: &str) -> Vec<Stmt> {
        let parsed = parse_module(source).expect("parse error");
        let mut module = parsed.into_syntax();
        let rewriter = RaiseRewriter::new();
        walk_body(&rewriter, &mut module.body);
        module.body
    }

    #[test]
    fn rewrites_raise_from() {
        let input = "raise ValueError from exc";
        let expected = "dp_intrinsics.raise_from(ValueError, exc)";
        let output = rewrite_raise(input);
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn does_not_rewrite_plain_raise() {
        let input = "raise ValueError";
        let expected = "raise ValueError";
        let output = rewrite_raise(input);
        assert_flatten_eq!(output, expected);
    }
}
