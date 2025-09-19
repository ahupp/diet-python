#[cfg(target_arch = "wasm32")]
use js_sys::{Array, Object, Reflect};
use ruff_python_ast::{self as ast, ModModule, Stmt};
use ruff_python_codegen::{Generator, Indentation};
use ruff_python_parser::{parse_module, ParseError};
use ruff_source_file::LineEnding;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsValue;

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy)]
enum TransformKind {
    InjectImport,
    LowerAttributes,
    Truthy,
}

#[cfg(target_arch = "wasm32")]
struct TransformToggle {
    id: &'static str,
    label: &'static str,
    default_enabled: bool,
    kind: TransformKind,
}

#[cfg(target_arch = "wasm32")]
const TRANSFORM_TOGGLES: &[TransformToggle] = &[
    TransformToggle {
        id: "inject_import",
        label: "Inject __dp__ import",
        default_enabled: true,
        kind: TransformKind::InjectImport,
    },
    TransformToggle {
        id: "lower_attributes",
        label: "Rewrite attribute access",
        default_enabled: true,
        kind: TransformKind::LowerAttributes,
    },
    TransformToggle {
        id: "truthiness",
        label: "Rewrite truthiness checks",
        default_enabled: false,
        kind: TransformKind::Truthy,
    },
];

pub mod body_transform;
pub mod ensure_import;
pub mod intrinsics;
pub mod min_ast;
mod template;
#[cfg(test)]
mod test_util;
mod transform;

use crate::body_transform::Transformer;
use transform::{context::Context, expr::ExprRewriter, Options};

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
    let mut expr_transformer = ExprRewriter::new(&ctx);
    expr_transformer.visit_body(&mut module.body);

    // Collapse `py_stmt!` templates after all rewrites.
    template::flatten(&mut module.body);

    if options.truthy {
        transform::rewrite_truthy::rewrite(&mut module.body);
    }

    strip_type_aliases(&mut module.body);
}

/// Transform the source code and return the resulting string.
fn transform_to_string_with_options(source: &str, options: Options) -> Result<String, ParseError> {
    let module = transform_str_to_ruff_with_options(source, options)?;
    Ok(ruff_ast_to_string(&module.body))
}

pub fn transform_to_string(source: &str, ensure: bool) -> Result<String, ParseError> {
    transform_to_string_with_options(
        source,
        Options {
            inject_import: ensure,
            ..Options::default()
        },
    )
}

fn strip_type_aliases(stmts: &mut Vec<Stmt>) {
    stmts.retain_mut(|stmt| match stmt {
        Stmt::FunctionDef(ast::StmtFunctionDef { ref mut body, .. })
        | Stmt::ClassDef(ast::StmtClassDef { ref mut body, .. }) => {
            strip_type_aliases(body);
            true
        }
        Stmt::For(ast::StmtFor {
            ref mut body,
            ref mut orelse,
            ..
        })
        | Stmt::While(ast::StmtWhile {
            ref mut body,
            ref mut orelse,
            ..
        }) => {
            strip_type_aliases(body);
            strip_type_aliases(orelse);
            true
        }
        Stmt::If(ast::StmtIf {
            ref mut body,
            ref mut elif_else_clauses,
            ..
        }) => {
            strip_type_aliases(body);
            for clause in elif_else_clauses {
                strip_type_aliases(&mut clause.body);
            }
            true
        }
        Stmt::With(ast::StmtWith { ref mut body, .. }) => {
            strip_type_aliases(body);
            true
        }
        Stmt::Try(ast::StmtTry {
            ref mut body,
            ref mut handlers,
            ref mut orelse,
            ref mut finalbody,
            ..
        }) => {
            strip_type_aliases(body);
            for handler in handlers {
                match handler {
                    ast::ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
                        ref mut body,
                        ..
                    }) => strip_type_aliases(body),
                }
            }
            strip_type_aliases(orelse);
            strip_type_aliases(finalbody);
            true
        }
        Stmt::Match(ast::StmtMatch { ref mut cases, .. }) => {
            for case in cases {
                strip_type_aliases(&mut case.body);
            }
            true
        }
        Stmt::TypeAlias(_) => false,
        _ => true,
    });
}

pub fn transform_to_string_without_attribute_lowering(
    source: &str,
    ensure: bool,
) -> Result<String, ParseError> {
    let module = transform_str_to_ruff_with_options(
        source,
        Options {
            inject_import: ensure,
            lower_attributes: false,
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
pub fn transform_selected(source: &str, transforms: Array) -> Result<String, JsValue> {
    let options = wasm_options_from_selected(&transforms);
    transform_to_string_with_options(source, options).map_err(|e| e.to_string().into())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn available_transforms() -> Array {
    let out = Array::new();
    for transform in TRANSFORM_TOGGLES {
        let obj = Object::new();
        Reflect::set(
            &obj,
            &JsValue::from_str("id"),
            &JsValue::from_str(transform.id),
        )
        .expect("id property set");
        Reflect::set(
            &obj,
            &JsValue::from_str("label"),
            &JsValue::from_str(transform.label),
        )
        .expect("label property set");
        Reflect::set(
            &obj,
            &JsValue::from_str("defaultEnabled"),
            &JsValue::from_bool(transform.default_enabled),
        )
        .expect("defaultEnabled property set");
        out.push(&obj.into());
    }
    out
}

#[cfg(target_arch = "wasm32")]
fn wasm_options_from_selected(transforms: &Array) -> Options {
    let selected: Vec<String> = transforms
        .iter()
        .filter_map(|value| value.as_string())
        .collect();
    let mut options = Options::default();
    for transform in TRANSFORM_TOGGLES {
        let enabled = selected.iter().any(|name| name == transform.id);
        match transform.kind {
            TransformKind::InjectImport => options.inject_import = enabled,
            TransformKind::LowerAttributes => options.lower_attributes = enabled,
            TransformKind::Truthy => options.truthy = enabled,
        }
    }
    options
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
