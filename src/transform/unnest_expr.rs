use std::cell::RefCell;

use ruff_python_ast::{Expr, Stmt};
use ruff_python_ast::visitor::transformer::{walk_expr, Transformer};

use super::lower::Context;

fn is_simple(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Name(_)
            | Expr::NumberLiteral(_)
            | Expr::StringLiteral(_)
            | Expr::BytesLiteral(_)
            | Expr::BooleanLiteral(_)
            | Expr::NoneLiteral(_)
            | Expr::EllipsisLiteral(_)
    )
}

pub struct UnnestExprTransformer<'a> {
    pub ctx: &'a Context,
    pub stmts: RefCell<Vec<Stmt>>,
}

impl<'a> UnnestExprTransformer<'a> {
    pub fn new(ctx: &'a Context) -> Self {
        Self { ctx, stmts: RefCell::new(Vec::new()) }
    }
}

impl<'a> Transformer for UnnestExprTransformer<'a> {
    fn visit_stmt(&self, _stmt: &mut Stmt) {
        // Do not recurse into nested statements
    }

    fn visit_expr(&self, expr: &mut Expr) {
        walk_expr(self, expr);
        if !is_simple(expr) {
            let tmp = self.ctx.namer.fresh("_dp_tmp");
            let value = expr.clone();
            let assign = crate::py_stmt!(
                "\n{tmp:id} = {expr:expr}\n",
                tmp = tmp.as_str(),
                expr = value,
            );
            self.stmts.borrow_mut().push(assign);
            *expr = crate::py_expr!(
                "\n{tmp:id}\n",
                tmp = tmp.as_str(),
            );
        }
    }
}

pub fn unnest_expr(ctx: &Context, mut expr: Expr) -> (Expr, Vec<Stmt>) {
    let transformer = UnnestExprTransformer::new(ctx);
    transformer.visit_expr(&mut expr);
    let stmts = transformer.stmts.take();
    (expr, stmts)
}

pub fn unnest_exprs(ctx: &Context, exprs: Vec<Expr>) -> (Vec<Expr>, Vec<Stmt>) {
    let mut out = Vec::new();
    let mut stmts = Vec::new();
    for expr in exprs {
        let (expr, mut s) = unnest_expr(ctx, expr);
        out.push(expr);
        stmts.append(&mut s);
    }
    (out, stmts)
}
