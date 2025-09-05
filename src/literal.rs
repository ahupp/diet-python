use ruff_python_ast::visitor::transformer::{walk_expr, Transformer};
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::TextRange;

pub struct LiteralRewriter;

impl LiteralRewriter {
    pub fn new() -> Self {
        Self
    }

    fn tuple_from(elts: Vec<Expr>) -> Expr {
        Expr::Tuple(ast::ExprTuple {
            node_index: ast::AtomicNodeIndex::default(),
            range: TextRange::default(),
            elts,
            ctx: ast::ExprContext::Load,
            parenthesized: false,
        })
    }
}

impl Transformer for LiteralRewriter {
    fn visit_expr(&self, expr: &mut Expr) {
        walk_expr(self, expr);
        match expr {
            Expr::List(ast::ExprList { elts, .. }) => {
                let tuple = Self::tuple_from(elts.clone());
                *expr = crate::py_expr!("list({tuple:expr})", tuple = tuple);
            }
            Expr::Set(ast::ExprSet { elts, .. }) => {
                let tuple = Self::tuple_from(elts.clone());
                *expr = crate::py_expr!("set({tuple:expr})", tuple = tuple);
            }
            Expr::Dict(ast::ExprDict { items, .. }) => {
                if items.iter().all(|item| item.key.is_some()) {
                    let pairs: Vec<Expr> = items
                        .iter()
                        .map(|item| {
                            let key = item.key.clone().unwrap();
                            let value = item.value.clone();
                            crate::py_expr!("({key:expr}, {value:expr})", key = key, value = value)
                        })
                        .collect();
                    let tuple = Self::tuple_from(pairs);
                    *expr = crate::py_expr!("dict({tuple:expr})", tuple = tuple);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruff_python_ast::visitor::transformer::walk_body;
    use ruff_python_codegen::{Generator, Stylist};
    use ruff_python_parser::parse_module;

    fn rewrite(source: &str) -> String {
        let parsed = parse_module(source).expect("parse error");
        let tokens = parsed.tokens().clone();
        let mut module = parsed.into_syntax();

        let rewriter = LiteralRewriter::new();
        walk_body(&rewriter, &mut module.body);

        let stylist = Stylist::from_tokens(&tokens, source);
        let mut output = String::new();
        for stmt in &module.body {
            let snippet = Generator::from(&stylist).stmt(stmt);
            output.push_str(&snippet);
            output.push_str(stylist.line_ending().as_str());
        }
        output
    }

    #[test]
    fn rewrites_list_literal() {
        let input = "a = [1, 2, 3]";
        let expected = "a = list((1, 2, 3))";
        let output = rewrite(input);
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_set_literal() {
        let input = "a = {1, 2, 3}";
        let expected = "a = set((1, 2, 3))";
        let output = rewrite(input);
        assert_eq!(output.trim(), expected.trim());
    }

    #[test]
    fn rewrites_dict_literal() {
        let input = "a = {'a': 1, 'b': 2}";
        let expected = "a = dict((('a', 1), ('b', 2)))";
        let output = rewrite(input);
        assert_eq!(output.trim(), expected.trim());
    }
}
