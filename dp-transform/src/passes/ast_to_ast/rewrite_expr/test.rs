use super::lower_expr_impl;
use crate::passes::ast_to_ast::context::Context;
use crate::Options;
use ruff_python_ast::Expr;
use ruff_python_parser::parse_expression;

fn parse_expr(source: &str) -> Expr {
    *parse_expression(source)
        .expect("parse should succeed")
        .into_syntax()
        .body
}

#[test]
#[should_panic(expected = "helper-scoped expr leaked to lower_expr")]
fn lower_expr_rejects_lambda() {
    let context = Context::new(Options::for_test(), "lambda x: x");
    let _ = lower_expr_impl(&context, parse_expr("lambda x: x"), false);
}

#[test]
#[should_panic(expected = "helper-scoped expr leaked to lower_expr")]
fn lower_expr_rejects_listcomp() {
    let context = Context::new(Options::for_test(), "[x for x in xs]");
    let _ = lower_expr_impl(&context, parse_expr("[x for x in xs]"), false);
}

#[test]
#[should_panic(expected = "expr-if leaked to lower_expr")]
fn lower_expr_rejects_expr_if() {
    let context = Context::new(Options::for_test(), "a if cond else b");
    let _ = lower_expr_impl(&context, parse_expr("a if cond else b"), false);
}

#[test]
#[should_panic(expected = "string template leaked to lower_expr")]
fn lower_expr_rejects_fstring() {
    let context = Context::new(Options::for_test(), "f\"{x}\"");
    let _ = lower_expr_impl(&context, parse_expr("f\"{x}\""), false);
}
