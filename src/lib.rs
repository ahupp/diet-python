#[cfg(target_arch = "wasm32")]
use js_sys::Array;
use ruff_python_ast::visitor::transformer::walk_body;
use ruff_python_ast::{ModModule, Stmt};
use ruff_python_codegen::{Generator, Indentation};
use ruff_python_parser::{parse_module, ParseError};
use ruff_source_file::LineEnding;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;

pub mod ensure_import;
pub mod intrinsics;
pub mod min_ast;
pub mod owned_transform;
mod template;
#[cfg(test)]
mod test_util;
mod transform;

use transform::{context::Context, expr::ExprRewriter, truthy::TruthyRewriter, Options};

fn should_skip(source: &str) -> bool {
    source
        .lines()
        .next()
        .is_some_and(|line| line.contains("diet-python: disabled"))
}

fn apply_transforms(module: &mut ModModule, options: Options) {
    // Lower `for` loops, expand generators and lambdas, and replace
    // `__dp__.<name>` calls with `getattr` in a single pass.
    let ctx = Context::new(options);
    let expr_transformer = ExprRewriter::new(&ctx);
    expr_transformer.rewrite_body(&mut module.body);

    // Collapse `py_stmt!` templates after all rewrites.
    template::flatten(&mut module.body);

    if options.truthy {
        let truthy_transformer = TruthyRewriter::new();
        walk_body(&truthy_transformer, &mut module.body);
    }
}

/// Transform the source code and return the resulting string.
pub fn transform_to_string(source: &str, ensure: bool) -> Result<String, ParseError> {
    let module = transform_str_to_ruff_with_options(
        source,
        Options {
            inject_import: ensure,
            ..Options::default()
        },
    )?;
    Ok(ruff_ast_to_string(&module.body))
}

pub fn transform_str_to_str_exec(source: &str) -> Result<String, ParseError> {
    if should_skip(source) {
        return Ok(source.to_string());
    }

    transform_to_string(source, true)
}

/// Transform the source code and return the resulting Ruff AST.
pub fn transform_str_to_ruff_with_options(
    source: &str,
    options: Options,
) -> Result<ModModule, ParseError> {
    let mut module = parse_module(source)?.into_syntax();
    apply_transforms(&mut module, options);
    if options.lower_attributes {
        let _ = min_ast::Module::from(module.clone());
    }
    if options.inject_import {
        ensure_import::ensure_import(&mut module);
    }
    Ok(module)
}

/// Transform the source code with default options and return the resulting Ruff AST.
pub fn transform_str_to_ruff(source: &str) -> Result<ModModule, ParseError> {
    transform_str_to_ruff_with_options(source, Options::default())
}

/// Convert a ruff AST ModModule to a pretty-printed string.
pub fn ruff_ast_to_string(module: &[Stmt]) -> String {
    // Use default stylist settings for pretty printing
    let indent = Indentation::new("    ".to_string());
    let mut output = String::new();
    for stmt in module {
        let gen = Generator::new(&indent, LineEnding::default());
        output.push_str(&gen.stmt(stmt));
        output.push_str(LineEnding::default().as_str());
    }
    output
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn transform(source: &str) -> Result<String, JsValue> {
    transform_to_string(source, true).map_err(|e| e.to_string().into())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn transform_selected(source: &str, _transforms: Array) -> Result<String, JsValue> {
    transform_to_string(source, true).map_err(|e| e.to_string().into())
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

    #[test]
    fn always_imports_dp() {
        let src = r#"
x = 1
"#;
        let result = transform_to_string(src, true).unwrap();
        assert!(result.contains("import __dp__"));
    }

    #[test]
    fn transform_string_rewrites_attribute_assign() {
        let src = r#"
a.b = 1
"#;
        let result = transform_to_string(src, true).unwrap();
        assert!(result.contains(r#"getattr(__dp__, "setattr")(a, "b", 1)"#));
        assert!(result.contains("import __dp__"));
    }
}
