use std::cell::{Cell, RefCell};

use super::context::Context;
use super::rewrite_expr_to_stmt::expr_boolop_to_stmts;
use crate::template::{is_simple, single_stmt};
use crate::{py_expr, py_stmt};
use ruff_python_ast::visitor::transformer::{walk_expr, walk_stmt, Transformer};
use ruff_python_ast::{Expr, Stmt};

pub(crate) struct UnnestExprTransformer<'a> {
    pub(crate) ctx: &'a Context,
    pub(crate) stmts: RefCell<Vec<Stmt>>,
}

impl<'a> UnnestExprTransformer<'a> {
    pub(crate) fn new(ctx: &'a Context) -> Self {
        Self {
            ctx,
            stmts: RefCell::new(Vec::new()),
        }
    }
}

impl<'a> Transformer for UnnestExprTransformer<'a> {
    fn visit_stmt(&self, _stmt: &mut Stmt) {
        // Do not recurse into nested statements
    }

    fn visit_expr(&self, expr: &mut Expr) {
        walk_expr(self, expr);
        if !is_simple(expr) {
            match expr {
                Expr::BoolOp(bool_op) => {
                    let tmp = self.ctx.fresh("tmp");
                    let stmts = expr_boolop_to_stmts(tmp.as_str(), bool_op.clone());
                    self.stmts.borrow_mut().extend(stmts);
                    *expr = py_expr!("{tmp:id}", tmp = tmp.as_str());
                }
                _ => {
                    let tmp = self.ctx.fresh("tmp");
                    let value = expr.clone();
                    let assign = py_stmt!(
                        "\n{tmp:id} = {expr:expr}\n",
                        tmp = tmp.as_str(),
                        expr = value,
                    );
                    self.stmts.borrow_mut().push(assign);
                    *expr = py_expr!("{tmp:id}\n", tmp = tmp.as_str());
                }
            }
        }
    }
}

pub(crate) struct UnnestTransformer<'a> {
    pub(crate) ctx: &'a Context,
}

impl<'a> UnnestTransformer<'a> {
    pub(crate) fn new(ctx: &'a Context) -> Self {
        Self { ctx }
    }
}

impl<'a> Transformer for UnnestTransformer<'a> {
    fn visit_stmt(&self, stmt: &mut Stmt) {
        let transformer = UnnestExprTransformer::new(self.ctx);
        walk_stmt(&transformer, stmt);
        walk_stmt(self, stmt);
        let mut stmts = transformer.stmts.take();
        if stmts.is_empty() {
            return;
        }
        // Package the hoisted temporaries alongside the rewritten statement so
        // parents only see a single statement to replace.
        stmts.push(stmt.clone());
        *stmt = single_stmt(stmts);
    }
}

#[allow(dead_code)]
pub(crate) struct ComplexExprTransformer {
    pub(crate) requires_unnest: Cell<bool>,
}

impl ComplexExprTransformer {
    #[allow(dead_code)]
    pub(crate) fn new() -> Self {
        Self {
            requires_unnest: Cell::new(false),
        }
    }
}

impl Transformer for ComplexExprTransformer {
    fn visit_expr(&self, expr: &mut Expr) {
        if matches!(expr, Expr::BoolOp(_)) {
            self.requires_unnest.set(true);
            return;
        }

        walk_expr(self, expr);
    }

    fn visit_stmt(&self, _stmt: &mut Stmt) {
        // We only want to handle expressions that are directly referenced by this stmt.
    }
}

#[allow(dead_code)]
pub(crate) fn rewrite(stmt: &mut Stmt, ctx: &Context) {
    let transformer = ComplexExprTransformer::new();
    walk_stmt(&transformer, stmt);
    if transformer.requires_unnest.get() {
        let unnest = UnnestTransformer::new(ctx);
        unnest.visit_stmt(stmt);
    }
}
