use std::collections::HashSet;

#[cfg(target_arch = "wasm32")]
use js_sys::Array;
use ruff_python_ast::visitor::transformer::walk_body;
use ruff_python_ast::{self as ast, Expr, ModModule, Pattern, Stmt};
use ruff_python_codegen::{Generator, Stylist};
use ruff_python_parser::{parse_module, ParseError};
use ruff_text_size::TextRange;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;

pub mod intrinsics;
pub mod min_ast;
pub mod owned_transform;
mod template;
#[cfg(test)]
mod test_util;
mod transform;

use transform::assert::AssertRewriter;
use transform::class_def::ClassDefRewriter;
use transform::decorator::DecoratorRewriter;
use transform::expr::ExprRewriter;
use transform::gen::GeneratorRewriter;
use transform::truthy::TruthyRewriter;
use transform::with::WithRewriter;

const TRANSFORM_NAMES: &[&str] = &[
    "gen",
    "with",
    "assert",
    "decorator",
    "class_def",
    "import",
    "truthy",
    "expr",
    "flatten",
];

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
    if run("with") {
        let with_transformer = WithRewriter::new();
        walk_body(&with_transformer, &mut module.body);
    }
    if run("assert") {
        let assert_transformer = AssertRewriter::new();
        walk_body(&assert_transformer, &mut module.body);
    }
    if run("decorator") {
        let decorator_transformer = DecoratorRewriter::new();
        walk_body(&decorator_transformer, &mut module.body);
    }
    if run("class_def") {
        let class_def_transformer = ClassDefRewriter::new();
        walk_body(&class_def_transformer, &mut module.body);
    }
    if run("import") {
        let import_rewriter = transform::import::ImportRewriter::new();
        walk_body(&import_rewriter, &mut module.body);
    }
    if run("truthy") {
        let truthy_transformer = TruthyRewriter::new();
        walk_body(&truthy_transformer, &mut module.body);
    }
    if run("expr") {
        let expr_transformer = ExprRewriter::new();
        walk_body(&expr_transformer, &mut module.body);
    }
    if run("gen") {
        let gen_transformer = GeneratorRewriter::new();
        gen_transformer.rewrite_body(&mut module.body);
    }
    if run("flatten") {
        // Previous transforms use `__dp__.<name>` calls; `expr` lowers them
        // to use `getattr`, so apply it before the final template flattening.
        template::flatten(&mut module.body);
    }
}

/// Convert a `Pattern` to source code.
///
/// Needed until `ruff_python_codegen` supports unparsing `Pattern::MatchClass`.
fn pattern_to_string(pattern: &Pattern, stylist: &Stylist) -> String {
    match pattern {
        Pattern::MatchClass(ast::PatternMatchClass { cls, arguments, .. }) => {
            let mut result = Generator::from(stylist).expr(cls);
            result.push('(');
            let mut first = true;
            for p in &arguments.patterns {
                if !first {
                    result.push_str(", ");
                } else {
                    first = false;
                }
                result.push_str(&pattern_to_string(p, stylist));
            }
            for kw in &arguments.keywords {
                if !first {
                    result.push_str(", ");
                } else {
                    first = false;
                }
                result.push_str(kw.attr.as_str());
                result.push('=');
                result.push_str(&pattern_to_string(&kw.pattern, stylist));
            }
            result.push(')');
            result
        }
        other => {
            // Use `Generator` to unparse supported patterns by constructing a dummy match statement.
            let dummy_case = ast::MatchCase {
                pattern: other.clone(),
                guard: None,
                body: vec![Stmt::Pass(ast::StmtPass {
                    range: TextRange::default(),
                    node_index: ast::AtomicNodeIndex::default(),
                })],
                range: TextRange::default(),
                node_index: ast::AtomicNodeIndex::default(),
            };
            let dummy_stmt = Stmt::Match(ast::StmtMatch {
                subject: Box::new(Expr::Name(ast::ExprName {
                    id: "x".into(),
                    ctx: ast::ExprContext::Load,
                    range: TextRange::default(),
                    node_index: ast::AtomicNodeIndex::default(),
                })),
                cases: vec![dummy_case],
                range: TextRange::default(),
                node_index: ast::AtomicNodeIndex::default(),
            });
            let snippet = Generator::from(stylist).stmt(&dummy_stmt);
            snippet
                .splitn(2, "case ")
                .nth(1)
                .and_then(|s| s.splitn(2, ":\n").next())
                .unwrap_or("")
                .to_string()
        }
    }
}

fn stmt_to_string(stmt: &Stmt, stylist: &Stylist) -> String {
    // Manual rendering to support `match` statements with class patterns.
    // Remove once `ruff_python_codegen` can unparse them directly.
    if let Stmt::Match(ast::StmtMatch { subject, cases, .. }) = stmt {
        let mut out = String::new();
        out.push_str("match ");
        out.push_str(&Generator::from(stylist).expr(subject));
        out.push_str(":");
        out.push_str(stylist.line_ending().as_str());
        for case in cases {
            out.push_str("    case ");
            out.push_str(&pattern_to_string(&case.pattern, stylist));
            if let Some(guard) = &case.guard {
                out.push_str(" if ");
                out.push_str(&Generator::from(stylist).expr(guard));
            }
            out.push_str(":");
            out.push_str(stylist.line_ending().as_str());
            for body_stmt in &case.body {
                let body = stmt_to_string(body_stmt, stylist);
                for line in body.lines() {
                    out.push_str("        ");
                    out.push_str(line);
                    out.push_str(stylist.line_ending().as_str());
                }
            }
        }
        let line_ending = stylist.line_ending().as_str();
        if out.ends_with(line_ending) {
            let len = line_ending.len();
            out.truncate(out.len() - len);
        }
        out
    } else {
        Generator::from(stylist).stmt(stmt)
    }
}

/// Transform the source code and return the resulting string.
pub fn transform_string(
    source: &str,
    transforms: Option<&HashSet<String>>,
) -> Result<String, ParseError> {
    if should_skip(source, transforms) {
        return Ok(source.to_string());
    }
    // Also transform to minimal AST to surface unsupported syntax panics
    let _ = transform_min_ast(source, transforms)?;

    let parsed = parse_module(source)?;
    let tokens = parsed.tokens().clone();
    let mut module = parsed.into_syntax();

    apply_transforms(&mut module, transforms);
    transform::import::ensure_import(&mut module, "__dp__");

    let stylist = Stylist::from_tokens(&tokens, source);
    let mut output = String::new();
    for stmt in &module.body {
        let snippet = stmt_to_string(stmt, &stylist);
        output.push_str(&snippet);
        output.push_str(stylist.line_ending().as_str());
    }
    Ok(output)
}

/// Transform the source code and return the resulting Ruff AST.
pub fn transform_ruff_ast(
    source: &str,
    transforms: Option<&HashSet<String>>,
) -> Result<ModModule, ParseError> {
    if should_skip(source, transforms) {
        return parse_module(source).map(|parsed| parsed.into_syntax());
    }

    let mut module = parse_module(source)?.into_syntax();
    apply_transforms(&mut module, transforms);
    Ok(module)
}

/// Transform the source code and return the resulting minimal AST.
pub fn transform_min_ast(
    source: &str,
    transforms: Option<&HashSet<String>>,
) -> Result<min_ast::Module, ParseError> {
    transform_ruff_ast(source, transforms).map(Into::into)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn transform(source: &str) -> Result<String, JsValue> {
    transform_string(source, None).map_err(|e| e.to_string().into())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn transform_selected(source: &str, transforms: Array) -> Result<String, JsValue> {
    let set: HashSet<String> = transforms.iter().filter_map(|v| v.as_string()).collect();
    transform_string(source, Some(&set)).map_err(|e| e.to_string().into())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn available_transforms() -> Array {
    TRANSFORM_NAMES
        .iter()
        .map(|&s| JsValue::from_str(s))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn none_means_all_transforms() {
        let src = "x = 1\n";
        let result = transform_string(src, None).unwrap();
        assert!(result.contains("import __dp__"));
    }

    #[test]
    fn empty_set_means_no_transforms() {
        let src = "x = 1\n";
        let set = HashSet::new();
        let result = transform_string(src, Some(&set)).unwrap();
        assert_eq!(result, src);
    }

    #[test]
    fn transform_string_rewrites_attribute_assign() {
        let src = r#"
a.b = 1
"#;
        let result = transform_string(src, None).unwrap();
        assert!(result.contains(r#"getattr(__dp__, "setattr")(a, "b", 1)"#));
    }
}
