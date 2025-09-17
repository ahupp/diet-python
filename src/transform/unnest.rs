use super::context::Context;
use super::rewrite_complex_expr::UnnestTransformer;
use ruff_python_ast::visitor::transformer::Transformer;
use ruff_python_ast::Stmt;

#[allow(dead_code)]
pub fn unnest_stmts(ctx: &Context, mut stmts: Vec<Stmt>) -> Vec<Stmt> {
    let transformer = UnnestTransformer::new(ctx);
    transformer.visit_body(&mut stmts);
    crate::template::flatten(&mut stmts);
    stmts
}

#[cfg(test)]
mod tests {
    use super::super::Options;
    use super::*;
    use crate::test_util::assert_ast_eq;
    use ruff_python_parser::parse_module;

    #[test]
    fn unnest_binop() {
        let input = r#"
a = (1 + 2) + (3 + 4)
"#;
        let module = parse_module(input).unwrap().into_syntax();
        let ctx = Context::new(Options::for_test());
        let body = unnest_stmts(&ctx, module.body);
        let expected = r#"
_dp_tmp_1 = 1 + 2
_dp_tmp_2 = 3 + 4
_dp_tmp_3 = _dp_tmp_1 + _dp_tmp_2
a = _dp_tmp_3
"#;
        let expected = parse_module(expected).unwrap().into_syntax();
        assert_ast_eq(&body, &expected.body);
    }
}
