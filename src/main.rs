use std::{env, fs, process};

use ruff_python_ast::visitor::transformer::{walk_body, Transformer};
use ruff_python_ast::{self as ast, Pattern, Stmt};
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
    if source
        .lines()
        .next()
        .is_some_and(|line| line.contains("diet-python: disabled"))
    {
        return source.to_string();
    }

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

    if ensure_import {
        import::ensure_import(&mut module, "dp_intrinsics");
    }

    let simple_expr_transformer = SimpleExprTransformer::new();
    walk_body(&simple_expr_transformer, &mut module.body);

    let literal_transformer = LiteralRewriter::new();
    walk_body(&literal_transformer, &mut module.body);

    let single_assign_transformer = SingleAssignmentRewriter::new();
    walk_body(&single_assign_transformer, &mut module.body);
    
    crate::template::flatten(&mut module.body);

    // Ruff's code generator doesn't support match class patterns with arguments.
    // If present, fall back to the original source.
    if contains_match_class(&mut module.body) {
        return source.to_string();
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

fn contains_match_class(body: &mut [Stmt]) -> bool {
    use std::cell::Cell;

    struct Finder {
        found: Cell<bool>,
    }

    impl Transformer for Finder {
        fn visit_stmt(&self, stmt: &mut Stmt) {
            if self.found.get() {
                return;
            }
            if let Stmt::Match(ast::StmtMatch { cases, .. }) = stmt {
                for case in cases {
                    if pattern_has_class(&case.pattern) {
                        self.found.set(true);
                        return;
                    }
                    for body_stmt in &mut case.body {
                        self.visit_stmt(body_stmt);
                        if self.found.get() {
                            return;
                        }
                    }
                }
            } else {
                ruff_python_ast::visitor::transformer::walk_stmt(self, stmt);
            }
        }
    }

    fn pattern_has_class(pattern: &Pattern) -> bool {
        use Pattern::*;
        match pattern {
            MatchClass(_) => true,
            MatchAs(ast::PatternMatchAs { pattern: Some(p), .. }) => pattern_has_class(p),
            MatchOr(ast::PatternMatchOr { patterns, .. }) => patterns.iter().any(pattern_has_class),
            MatchSequence(ast::PatternMatchSequence { patterns, .. }) => patterns.iter().any(pattern_has_class),
            MatchMapping(ast::PatternMatchMapping { patterns, .. }) => patterns.iter().any(pattern_has_class),
            _ => false,
        }
    }

    let finder = Finder {
        found: Cell::new(false),
    };
    for stmt in body.iter_mut() {
        finder.visit_stmt(stmt);
        if finder.found.get() {
            return true;
        }
    }
    false
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
