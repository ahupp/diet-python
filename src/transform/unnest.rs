use ruff_python_ast::visitor::transformer::{walk_stmt, Transformer};
use ruff_python_ast::Stmt;

use std::cell::{Cell, RefCell};

use ruff_python_ast::visitor::transformer::walk_expr;
use ruff_python_ast::Expr;

use crate::transform::Options;

pub struct Namer {
    pub counter: Cell<usize>,
}

impl Namer {
    pub fn new() -> Self {
        Self {
            counter: Cell::new(0),
        }
    }

    pub fn fresh(&self, prefix: &str) -> String {
        let id = self.counter.get();
        self.counter.set(id + 1);
        format!("{prefix}_{id}")
    }
}

pub struct Context {
    pub namer: Namer,
    pub options: Options,
}

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
            let tmp = self.ctx.namer.fresh("_dp_tmp");
            let value = expr.clone();
            let assign = crate::py_stmt!(
                "\n{tmp:id} = {expr:expr}\n",
                tmp = tmp.as_str(),
                expr = value,
            );
            self.stmts.borrow_mut().push(assign);
            *expr = crate::py_expr!("\n{tmp:id}\n", tmp = tmp.as_str(),);
        }
    }
}

pub struct UnnestTransformer<'a> {
    pub ctx: &'a Context,
}

impl<'a> UnnestTransformer<'a> {
    pub fn new(ctx: &'a Context) -> Self {
        Self { ctx }
    }

    pub fn visit_stmts(&self, body: &mut Vec<Stmt>) {
        let mut result = Vec::new();
        for mut stmt in std::mem::take(body) {
            let transformer = UnnestExprTransformer::new(self.ctx);
            walk_stmt(&transformer, &mut stmt);
            walk_stmt(self, &mut stmt);
            let mut stmts = transformer.stmts.take();
            result.append(&mut stmts);
            result.push(stmt);
        }
        *body = result;
    }
}

impl<'a> Transformer for UnnestTransformer<'a> {}

pub fn unnest_stmts(ctx: &Context, mut stmts: Vec<Stmt>) -> Vec<Stmt> {
    let transformer = UnnestTransformer::new(ctx);
    transformer.visit_stmts(&mut stmts);
    stmts
}

#[cfg(test)]
mod tests {
    use super::super::Options;
    use super::Namer;
    use super::*;
    use crate::test_util::assert_ast_eq;
    use ruff_python_parser::parse_module;

    #[test]
    fn unnest_binop() {
        let input = r#"
a = (1 + 2) + (3 + 4)
"#;
        let module = parse_module(input).unwrap().into_syntax();
        let ctx = Context {
            namer: Namer::new(),
            options: Options::for_test(),
        };
        let body = unnest_stmts(&ctx, module.body);
        let expected = r#"
_dp_tmp_0 = 1 + 2
_dp_tmp_1 = 3 + 4
_dp_tmp_2 = _dp_tmp_0 + _dp_tmp_1
a = _dp_tmp_2
"#;
        let expected = parse_module(expected).unwrap().into_syntax();
        assert_ast_eq(&body, &expected.body);
    }
}
