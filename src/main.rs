use std::{env, fs, process};

use ruff_python_ast::visitor::transformer::walk_body;
use ruff_python_codegen::{Generator, Stylist};
use ruff_python_parser::parse_module;

mod comprehension;
mod for_loop;
mod gen;
mod import;
mod assert;
mod literal;
mod multi_target;
mod operator;
mod simple_expr;
mod template;
#[cfg(test)]
mod test_util;
mod with;
mod single_assignment;

use for_loop::ForLoopRewriter;
use gen::GeneratorRewriter;
use literal::LiteralRewriter;
use multi_target::MultiTargetRewriter;
use operator::OperatorRewriter;
use simple_expr::SimpleExprTransformer;
use with::WithRewriter;
use assert::AssertRewriter;
use single_assignment::SingleAssignmentRewriter;

fn rewrite_source_inner(source: &str, ensure_import: bool) -> String {
    let parsed = parse_module(source).expect("parse error");
    let tokens = parsed.tokens().clone();
    let mut module = parsed.into_syntax();

    let gen_transformer = GeneratorRewriter::new();
    gen_transformer.rewrite_body(&mut module.body);

    let with_transformer = WithRewriter::new();
    walk_body(&with_transformer, &mut module.body);

    let for_transformer = ForLoopRewriter::new();
    walk_body(&for_transformer, &mut module.body);

    let multi_transformer = MultiTargetRewriter::new();
    walk_body(&multi_transformer, &mut module.body);

    let assert_transformer = AssertRewriter::new();
    walk_body(&assert_transformer, &mut module.body);

    let op_transformer = OperatorRewriter::new();
    walk_body(&op_transformer, &mut module.body);

    if ensure_import && op_transformer.transformed() {
        import::ensure_import(&mut module, "operator");
    }
    if ensure_import && with_transformer.transformed() {
        import::ensure_import(&mut module, "sys");
    }

    let simple_expr_transformer = SimpleExprTransformer::new();
    walk_body(&simple_expr_transformer, &mut module.body);

    let literal_transformer = LiteralRewriter::new();
    walk_body(&literal_transformer, &mut module.body);

    let single_assign_transformer = SingleAssignmentRewriter::new();
    walk_body(&single_assign_transformer, &mut module.body);

    crate::template::flatten(&mut module.body);

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
