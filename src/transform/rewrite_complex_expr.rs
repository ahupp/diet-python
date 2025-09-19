use std::cell::RefCell;

use super::context::Context;
use crate::body_transform::{walk_expr, walk_stmt, Transformer};
use crate::template::{is_simple, single_stmt};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt};

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
    fn visit_stmt(&mut self, _stmt: &mut Stmt) {
        // Do not recurse into nested statements
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        walk_expr(self, expr);
        if !is_simple(expr) {
            match expr {
                Expr::YieldFrom(yield_from) => {
                    let state_name = self.ctx.fresh("yield_from_state");
                    let sent_name = self.ctx.fresh("yield_from_sent");
                    let result_name = self.ctx.fresh("tmp");
                    let ast::ExprYieldFrom { value, .. } = yield_from.clone();
                    let iterable = *value;
                    let driver = py_stmt!(
                        r#"
{state:id} = __dp__.yield_from_init({iterable:expr})
{sent:id} = None
while True:
    if __dp__.getitem({state:id}, 0) != __dp__.RUNNING:
        break
    try:
        {sent:id} = yield __dp__.getitem({state:id}, 1)
    except:
        {state:id} = __dp__.yield_from_except({state:id}, __dp__.current_exception())
    else:
        {state:id} = __dp__.yield_from_next({state:id}, {sent:id})
{result:id} = __dp__.getitem({state:id}, 1)
"#,
                        state = state_name.as_str(),
                        sent = sent_name.as_str(),
                        result = result_name.as_str(),
                        iterable = iterable,
                    );
                    self.stmts.borrow_mut().push(driver);
                    *expr = py_expr!("{result:id}", result = result_name.as_str());
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
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        let mut transformer = UnnestExprTransformer::new(self.ctx);
        walk_stmt(&mut transformer, stmt);
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
    pub(crate) requires_unnest: bool,
}

impl ComplexExprTransformer {
    #[allow(dead_code)]
    pub(crate) fn new() -> Self {
        Self {
            requires_unnest: false,
        }
    }
}

impl Transformer for ComplexExprTransformer {
    fn visit_expr(&mut self, expr: &mut Expr) {
        if matches!(expr, Expr::YieldFrom(_)) {
            self.requires_unnest = true;
            return;
        }

        walk_expr(self, expr);
    }

    fn visit_stmt(&mut self, _stmt: &mut Stmt) {
        // We only want to handle expressions that are directly referenced by this stmt.
    }
}

#[allow(dead_code)]
pub(crate) fn rewrite(stmt: &mut Stmt, ctx: &Context) {
    let mut transformer = ComplexExprTransformer::new();
    walk_stmt(&mut transformer, stmt);
    if transformer.requires_unnest {
        let mut unnest = UnnestTransformer::new(ctx);
        unnest.visit_stmt(stmt);
    }
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_complex_expr.txt");
}
