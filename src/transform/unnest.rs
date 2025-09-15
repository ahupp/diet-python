use ruff_python_ast::visitor::transformer::{walk_stmt, Transformer};
use ruff_python_ast::Stmt;

use super::lower::Context;
use super::unnest_expr::UnnestExprTransformer;

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
    use super::*;
    use crate::test_util::assert_ast_eq;
    use ruff_python_parser::parse_module;
    use super::super::lower::{Namer};
    use super::super::Options;

    #[test]
    fn unnest_binop() {
        let input = r#"
a = (1 + 2) + (3 + 4)
"#;
        let module = parse_module(input).unwrap().into_syntax();
        let ctx = Context { namer: Namer::new(), options: Options::for_test() };
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
