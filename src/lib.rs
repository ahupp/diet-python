use std::collections::HashSet;

use ruff_python_ast::visitor::transformer::{walk_body, Transformer};
use ruff_python_ast::{self as ast, Mod, ModModule, Pattern, Stmt};
use ruff_python_codegen::{Generator, Stylist};
use ruff_python_parser::parse_module;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

mod assert;
mod class_def;
mod comprehension;
mod decorator;
mod for_loop;
mod gen;
mod import;
mod literal;
mod multi_target;
mod operator;
mod raise;
mod simple_expr;
mod template;
#[cfg(test)]
mod test_util;
mod with;

use assert::AssertRewriter;
use class_def::ClassDefRewriter;
use decorator::DecoratorRewriter;
use for_loop::ForLoopRewriter;
use gen::GeneratorRewriter;
use literal::LiteralRewriter;
use multi_target::MultiTargetRewriter;
use operator::OperatorRewriter;
use raise::RaiseRewriter;
use simple_expr::SimpleExprTransformer;
use with::WithRewriter;

/// Parse the `DIET_PYTHON_TRANSFORMS` environment variable into a set of
/// transform names. Returns `None` if the variable is unset, meaning all
/// transforms should be applied.
pub fn parse_transforms() -> Option<HashSet<String>> {
    std::env::var("DIET_PYTHON_TRANSFORMS").ok().map(|value| {
        value
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    })
}

fn should_skip(source: &str, transforms: Option<&HashSet<String>>) -> bool {
    source
        .lines()
        .next()
        .is_some_and(|line| line.contains("diet-python: disabled"))
        || transforms.is_some_and(|t| t.is_empty())
}

fn apply_transforms(module: &mut ModModule, transforms: Option<&HashSet<String>>) {
    let run = |name: &str| transforms.map_or(true, |t| t.contains(name));
    if run("gen") {
        let gen_transformer = GeneratorRewriter::new();
        gen_transformer.rewrite_body(&mut module.body);
    }
    if run("with") {
        let with_transformer = WithRewriter::new();
        walk_body(&with_transformer, &mut module.body);
    }
    if run("for_loop") {
        let for_transformer = ForLoopRewriter::new();
        walk_body(&for_transformer, &mut module.body);
    }
    if run("multi_target") {
        let multi_transformer = MultiTargetRewriter::new();
        walk_body(&multi_transformer, &mut module.body);
    }
    if run("assert") {
        let assert_transformer = AssertRewriter::new();
        walk_body(&assert_transformer, &mut module.body);
    }
    if run("raise") {
        let raise_transformer = RaiseRewriter::new();
        walk_body(&raise_transformer, &mut module.body);
    }
    if run("decorator") {
        let decorator_transformer = DecoratorRewriter::new();
        walk_body(&decorator_transformer, &mut module.body);
    }
    if run("class_def") {
        let class_def_transformer = ClassDefRewriter::new();
        walk_body(&class_def_transformer, &mut module.body);
    }
    if run("multi_target") {
        let multi_transformer = MultiTargetRewriter::new();
        walk_body(&multi_transformer, &mut module.body);
    }
    if run("operator") {
        let op_transformer = OperatorRewriter::new();
        walk_body(&op_transformer, &mut module.body);
    }
    if run("simple_expr") {
        let simple_expr_transformer = SimpleExprTransformer::new();
        walk_body(&simple_expr_transformer, &mut module.body);
    }
    if run("literal") {
        let literal_transformer = LiteralRewriter::new();
        walk_body(&literal_transformer, &mut module.body);
    }
    if run("import") {
        let import_rewriter = import::ImportRewriter::new();
        walk_body(&import_rewriter, &mut module.body);
    }
    if run("flatten") {
        crate::template::flatten(&mut module.body);
    }
    import::ensure_import(module, "dp_intrinsics");
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
            MatchAs(ast::PatternMatchAs {
                pattern: Some(p), ..
            }) => pattern_has_class(p),
            MatchOr(ast::PatternMatchOr { patterns, .. }) => patterns.iter().any(pattern_has_class),
            MatchSequence(ast::PatternMatchSequence { patterns, .. }) => {
                patterns.iter().any(pattern_has_class)
            }
            MatchMapping(ast::PatternMatchMapping { patterns, .. }) => {
                patterns.iter().any(pattern_has_class)
            }
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

/// Transform the source code and return the resulting string.
pub fn transform_string(source: &str, transforms: Option<&HashSet<String>>) -> String {
    if should_skip(source, transforms) {
        return source.to_string();
    }

    let parsed = parse_module(source).expect("parse error");
    let tokens = parsed.tokens().clone();
    let mut module = parsed.into_syntax();

    apply_transforms(&mut module, transforms);

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

/// Transform the source code and return the resulting Ruff AST.
pub fn transform_ruff_ast(source: &str, transforms: Option<&HashSet<String>>) -> ModModule {
    if should_skip(source, transforms) {
        return parse_module(source).expect("parse error").into_syntax();
    }

    let mut module = parse_module(source).expect("parse error").into_syntax();
    apply_transforms(&mut module, transforms);
    if contains_match_class(&mut module.body) {
        return parse_module(source).expect("parse error").into_syntax();
    }
    module
}

/// Transform the source code and return the resulting minimal AST.
pub fn transform_min_ast(source: &str, transforms: Option<&HashSet<String>>) -> Mod {
    transform_ruff_ast(source, transforms).into()
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn transform(source: &str) -> String {
    transform_string(source, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn none_means_all_transforms() {
        let src = "x = 1\n";
        let result = transform_string(src, None);
        assert!(result.contains("import dp_intrinsics"));
    }

    #[test]
    fn empty_set_means_no_transforms() {
        let src = "x = 1\n";
        let set = HashSet::new();
        let result = transform_string(src, Some(&set));
        assert_eq!(result, src);
    }
}

