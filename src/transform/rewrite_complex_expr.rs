use std::cell::{Cell, RefCell};

use super::context::Context;
use super::rewrite_expr_to_stmt::{expr_boolop_to_stmts, expr_compare_to_stmts};
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
                Expr::Named(named_expr) => {
                    let tmp = self.ctx.fresh("tmp");
                    let ast::ExprNamed { target, value, .. } = named_expr.clone();
                    let assign_tmp = py_stmt!(
                        "\n{tmp:id} = {value:expr}\n",
                        tmp = tmp.as_str(),
                        value = *value,
                    );
                    let assign_target = py_stmt!(
                        "\n{target:expr} = {tmp:id}\n",
                        target = *target,
                        tmp = tmp.as_str(),
                    );
                    let mut stmts = self.stmts.borrow_mut();
                    stmts.push(assign_tmp);
                    stmts.push(assign_target);
                    *expr = py_expr!("{tmp:id}", tmp = tmp.as_str());
                }
                Expr::If(if_expr) => {
                    let tmp = self.ctx.fresh("tmp");
                    let ast::ExprIf {
                        test, body, orelse, ..
                    } = if_expr.clone();
                    let assign = py_stmt!(
                        "\nif {cond:expr}:\n    {tmp:id} = {body:expr}\nelse:\n    {tmp:id} = {orelse:expr}",
                        cond = *test,
                        tmp = tmp.as_str(),
                        body = *body,
                        orelse = *orelse,
                    );
                    self.stmts.borrow_mut().push(assign);
                    *expr = py_expr!("{tmp:id}", tmp = tmp.as_str());
                }
                Expr::Compare(compare) => {
                    let tmp = self.ctx.fresh("tmp");
                    let stmts = expr_compare_to_stmts(tmp.as_str(), compare.clone());
                    self.stmts.borrow_mut().extend(stmts);
                    *expr = py_expr!("{tmp:id}", tmp = tmp.as_str());
                }
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
        if matches!(
            expr,
            Expr::BoolOp(_) | Expr::If(_) | Expr::Compare(_) | Expr::YieldFrom(_)
        ) {
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

#[cfg(test)]
mod tests {
    use crate::test_util::assert_transform_eq;

    #[test]
    fn rewrites_yield_from_expression() {
        let input = r#"
x = yield from y
"#;
        let expected = r#"
_dp_yield_from_state_1 = __dp__.yield_from_init(y)
_dp_yield_from_sent_2 = None
while True:
    _dp_tmp_4 = __dp__.getitem
    _dp_tmp_5 = _dp_tmp_4(_dp_yield_from_state_1, 0)
    _dp_tmp_6 = __dp__.RUNNING
    _dp_tmp_7 = __dp__.ne(_dp_tmp_5, _dp_tmp_6)
    if _dp_tmp_7:
        break
    try:
        _dp_yield_from_sent_2 = yield __dp__.getitem(_dp_yield_from_state_1, 1)
    except:
        _dp_yield_from_state_1 = __dp__.yield_from_except(_dp_yield_from_state_1, __dp__.current_exception())
    else:
        _dp_yield_from_state_1 = __dp__.yield_from_next(_dp_yield_from_state_1, _dp_yield_from_sent_2)
_dp_tmp_3 = __dp__.getitem(_dp_yield_from_state_1, 1)
x = _dp_tmp_3
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn rewrites_named_expression_in_boolop() {
        let input = r#"
if (y := foo()) and bar:
    pass
"#;
        let expected = r#"
_dp_tmp_1 = foo()
_dp_tmp_2 = _dp_tmp_1
y = _dp_tmp_2
_dp_tmp_3 = _dp_tmp_2
if _dp_tmp_3:
    _dp_tmp_3 = bar
if _dp_tmp_3:
    pass
"#;
        assert_transform_eq(input, expected);
    }
}
