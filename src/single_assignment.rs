use std::cell::{Cell, RefCell};

use ruff_python_ast::visitor::transformer::{walk_expr, walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Expr, Stmt};

/// Evaluate non-trivial expressions once, storing them in temporary variables.
pub struct SingleAssignmentRewriter {
    tmp_count: Cell<usize>,
    stmts: RefCell<Vec<Vec<Stmt>>>,
}

impl SingleAssignmentRewriter {
    pub fn new() -> Self {
        Self {
            tmp_count: Cell::new(0),
            stmts: RefCell::new(Vec::new()),
        }
    }

    fn next_tmp(&self) -> String {
        let id = self.tmp_count.get() + 1;
        self.tmp_count.set(id);
        format!("_dp_tmp_{}", id)
    }

    fn store(&self, value: Expr) -> Expr {
        let name = self.next_tmp();
        let assign = crate::py_stmt!(
            "{name:id} = {value:expr}",
            name = name.as_str(),
            value = value,
        );
        self.stmts
            .borrow_mut()
            .last_mut()
            .expect("no statement context")
            .push(assign);
        crate::py_expr!("{name:id}", name = name.as_str())
    }

    fn is_trivial(expr: &Expr) -> bool {
        matches!(
            expr,
            Expr::Name(_)
                | Expr::NumberLiteral(_)
                | Expr::StringLiteral(_)
                | Expr::BooleanLiteral(_)
                | Expr::NoneLiteral(_)
        )
    }

    fn in_store_context(expr: &Expr) -> bool {
        use ast::ExprContext;
        match expr {
            Expr::Attribute(ast::ExprAttribute { ctx, .. })
            | Expr::Subscript(ast::ExprSubscript { ctx, .. })
            | Expr::List(ast::ExprList { ctx, .. })
            | Expr::Tuple(ast::ExprTuple { ctx, .. })
            | Expr::Name(ast::ExprName { ctx, .. })
            | Expr::Starred(ast::ExprStarred { ctx, .. }) => !matches!(ctx, ExprContext::Load),
            _ => false,
        }
    }
}

impl Transformer for SingleAssignmentRewriter {
    fn visit_stmt(&self, stmt: &mut Stmt) {
        self.stmts.borrow_mut().push(Vec::new());
        walk_stmt(self, stmt);
        let mut prepends = self.stmts.borrow_mut().pop().unwrap();
        if !prepends.is_empty() {
            prepends.push(stmt.clone());
            *stmt = crate::py_stmt!("{body:stmt}", body = prepends);
        }
    }

    fn visit_expr(&self, expr: &mut Expr) {
        walk_expr(self, expr);
        if matches!(expr, Expr::Starred(_)) || Self::in_store_context(expr) || Self::is_trivial(expr) {
            return;
        }
        let tmp = self.store(expr.clone());
        *expr = tmp;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_flatten_eq;
    use ruff_python_ast::visitor::transformer::walk_body;
    use ruff_python_parser::parse_module;

    fn rewrite(source: &str) -> Vec<Stmt> {
        let parsed = parse_module(source).expect("parse error");
        let mut module = parsed.into_syntax();
        let rewriter = SingleAssignmentRewriter::new();
        walk_body(&rewriter, &mut module.body);
        module.body
    }

    #[test]
    fn leaves_trivial_exprs() {
        let output = rewrite("x");
        assert_flatten_eq!(output, "x");
    }

    #[test]
    fn rewrites_calls_and_args() {
        let output = rewrite("(lambda x: x)(g(), h)");
        let expected = "_dp_tmp_1 = (lambda x: x)\n_dp_tmp_2 = g()\n_dp_tmp_3 = _dp_tmp_1(_dp_tmp_2, h)\n_dp_tmp_3";
        assert_flatten_eq!(output, expected);
    }

    #[test]
    fn rewrites_binary_ops() {
        let output = rewrite("r = a + b * c");
        let expected = "_dp_tmp_1 = b * c\n_dp_tmp_2 = a + _dp_tmp_1\nr = _dp_tmp_2";
        assert_flatten_eq!(output, expected);
    }
}

