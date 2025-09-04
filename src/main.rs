use std::{env, fs, process};

use ruff_python_ast::visitor::transformer::walk_body;
use ruff_python_codegen::{Generator, Stylist};
use ruff_python_parser::parse_module;

mod comprehension;
mod gen;
mod operator;

use gen::GeneratorRewriter;
use operator::{ensure_operator_import, OperatorRewriter};

fn rewrite_source_inner(source: &str, ensure_import: bool) -> String {
    let parsed = parse_module(source).expect("parse error");
    let tokens = parsed.tokens().clone();
    let mut module = parsed.into_syntax();

    let gen_transformer = GeneratorRewriter::new();
    gen_transformer.rewrite_body(&mut module.body);

    let op_transformer = OperatorRewriter::new();
    walk_body(&op_transformer, &mut module.body);

    if ensure_import && op_transformer.transformed() {
        ensure_operator_import(&mut module);
    }

    let stylist = Stylist::from_tokens(&tokens, source);
    let mut output = String::new();
    for stmt in &module.body {
        let snippet = Generator::from(&stylist).stmt(stmt);
        output.push_str(&snippet);
        output.push_str(stylist.line_ending().as_str());
    }
    output
}

fn transform_source(source: &str) -> String {
    rewrite_source_inner(source, true)
}

#[cfg(test)]
fn rewrite_source(source: &str) -> String {
    rewrite_source_inner(source, false)
}

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: diet-python <python-file>");
        process::exit(1);
    });

    let source = match fs::read_to_string(&path) {
        Ok(src) => src,
        Err(err) => {
            eprintln!("failed to read {}: {}", path, err);
            process::exit(1);
        }
    };

    let output = transform_source(&source);
    print!("{}", output);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_binary_ops() {
        let cases = [
            ("a + b", "operator.add(a, b)"),
            ("a - b", "operator.sub(a, b)"),
        ];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim_end(), expected);
        }
    }

    #[test]
    fn rewrites_aug_assign() {
        let input = "x = 1\nx += 2";
        let expected = "x = 1\nx = operator.iadd(x, 2)";
        let output = rewrite_source(input);
        assert_eq!(output.trim_end(), expected);
    }

    #[test]
    fn rewrites_unary_ops() {
        let cases = [
            ("-a", "operator.neg(a)"),
            ("~b", "operator.invert(b)"),
            ("not c", "operator.not_(c)"),
        ];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim_end(), expected);
        }
    }

    #[test]
    fn rewrites_comparisons() {
        let cases = [
            ("a == b", "operator.eq(a, b)"),
            ("a != b", "operator.ne(a, b)"),
            ("a < b", "operator.lt(a, b)"),
            ("a > b", "operator.gt(a, b)"),
            ("a is not b", "operator.is_not(a, b)"),
            ("a in b", "operator.contains(b, a)"),
            ("a not in b", "operator.not_(operator.contains(b, a))"),
        ];

        for (input, expected) in cases {
            let output = rewrite_source(input);
            assert_eq!(output.trim_end(), expected);
        }
    }
}
