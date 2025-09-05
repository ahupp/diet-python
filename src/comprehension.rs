use ruff_python_ast::visitor::transformer::Transformer;
use ruff_python_ast::{self as ast, Expr};
use ruff_text_size::TextRange;

pub(crate) fn rewrite_comprehension<T: Transformer>(transformer: &T, expr: &mut Expr) -> bool {
    let (elt, generators, func_name) = match expr {
        Expr::ListComp(ast::ExprListComp {
            elt, generators, ..
        }) => ((*elt.clone()), generators.clone(), "list"),
        Expr::SetComp(ast::ExprSetComp {
            elt, generators, ..
        }) => ((*elt.clone()), generators.clone(), "set"),
        Expr::DictComp(ast::ExprDictComp {
            key,
            value,
            generators,
            ..
        }) => {
            let tuple = crate::py_expr!(
                "({key}, {value})",
                key = (*key.clone()),
                value = (*value.clone())
            );
            (tuple, generators.clone(), "dict")
        }
        _ => return false,
    };

    let mut gen_expr = Expr::Generator(ast::ExprGenerator {
        node_index: ast::AtomicNodeIndex::default(),
        range: TextRange::default(),
        elt: Box::new(elt),
        generators,
        parenthesized: false,
    });

    transformer.visit_expr(&mut gen_expr);

    *expr = crate::py_expr!("{func}({gen})", gen = gen_expr; id func = func_name);

    true
}

#[cfg(test)]
mod tests {
    use crate::gen::GeneratorRewriter;
    use ruff_python_codegen::{Generator as Codegen, Stylist};
    use ruff_python_parser::parse_module;

    fn rewrite_gen(source: &str) -> String {
        let parsed = parse_module(source).expect("parse error");
        let tokens = parsed.tokens().clone();
        let mut module = parsed.into_syntax();

        let rewriter = GeneratorRewriter::new();
        rewriter.rewrite_body(&mut module.body);

        let stylist = Stylist::from_tokens(&tokens, source);
        let mut output = String::new();
        for stmt in &module.body {
            let snippet = Codegen::from(&stylist).stmt(stmt);
            output.push_str(&snippet);
            output.push_str(stylist.line_ending().as_str());
        }
        output
    }

    #[test]
    fn rewrites_list_comp() {
        let input = "r = [a + 1 for a in items if a % 2 == 0]";
        let expected = concat!(
            "def __dp_gen_1(items):\n",
            "    for a in items:\n",
            "        if a % 2 == 0:\n",
            "            yield a + 1\n",
            "r = list(__dp_gen_1(items))",
        );
        let output = rewrite_gen(input);
        assert_eq!(output.trim_end(), expected.trim_end());
    }

    #[test]
    fn rewrites_set_comp() {
        let input = "r = {a for a in items}";
        let expected = concat!(
            "def __dp_gen_1(items):\n",
            "    for a in items:\n",
            "        yield a\n",
            "r = set(__dp_gen_1(items))",
        );
        let output = rewrite_gen(input);
        assert_eq!(output.trim_end(), expected.trim_end());
    }

    #[test]
    fn rewrites_dict_comp() {
        let input = "r = {k: v + 1 for k, v in items if k % 2 == 0}";
        let expected = concat!(
            "def __dp_gen_1(items):\n",
            "    for k, v in items:\n",
            "        if k % 2 == 0:\n",
            "            yield k, v + 1\n",
            "r = dict(__dp_gen_1(items))",
        );
        let output = rewrite_gen(input);
        assert_eq!(output.trim_end(), expected.trim_end());
    }
}
