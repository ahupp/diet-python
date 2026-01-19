#[cfg(target_arch = "wasm32")]
use js_sys::{Array, Object, Reflect};
use ruff_python_ast::{ModModule, Stmt};
use ruff_python_codegen::{Generator, Indentation};
use ruff_python_parser::parse_module;
pub use ruff_python_parser::ParseError;
use ruff_source_file::LineEnding;
use std::time::{Duration, Instant};
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
pub mod fixture;
pub mod min_ast;
mod template;
#[cfg(test)]
mod test_util;
mod transform;

use crate::body_transform::Transformer;
pub use transform::{ImportStarHandling, Options};
use transform::{context::Context, driver::ExprRewriter};

#[derive(Debug, Clone, Copy)]
pub struct TransformTimings {
    pub parse: Duration,
    pub rewrite: Duration,
    pub ensure_import: Duration,
    pub emit: Duration,
    pub total: Duration,
}

fn should_skip(source: &str) -> bool {
    source
        .lines()
        .next()
        .is_some_and(|line| line.contains("diet-python: disabled"))
        || contains_surrogate_escape(source)
}

fn contains_surrogate_escape(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut backslashes = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'\\' {
            backslashes += 1;
            index += 1;
            continue;
        }
        if (byte == b'u' || byte == b'U') && backslashes % 2 == 1 {
            let digits = if byte == b'u' { 4 } else { 8 };
            if index + digits < bytes.len() {
                let mut value = 0u32;
                let mut valid = true;
                for offset in 0..digits {
                    let hex = bytes[index + 1 + offset];
                    let nibble = match hex {
                        b'0'..=b'9' => (hex - b'0') as u32,
                        b'a'..=b'f' => (hex - b'a' + 10) as u32,
                        b'A'..=b'F' => (hex - b'A' + 10) as u32,
                        _ => {
                            valid = false;
                            break;
                        }
                    };
                    value = (value << 4) | nibble;
                }
                if valid && (0xD800..=0xDFFF).contains(&value) {
                    return true;
                }
            }
        }
        backslashes = 0;
        index += 1;
    }
    false
}

fn apply_transforms(module: &mut ModModule, options: Options, source: &str) {
    let ctx = Context::new(options, source);
    transform::rewrite_future_annotations::rewrite(&mut module.body);

    // Lower `for` loops, expand generators and lambdas, and replace
    // `__dp__.<name>` calls with `getattr` in a single pass.
    let mut expr_transformer = ExprRewriter::new(ctx);
    expr_transformer.visit_body(&mut module.body);

    // Collapse `py_stmt!` templates after all rewrites.
    template::flatten(&mut module.body);

    transform::rewrite_explicit_scope::rewrite(&mut module.body);

    if options.truthy {
        transform::simple::rewrite_truthy::rewrite(&mut module.body);
    }

    strip_generated_passes(&mut module.body);
}

/// Transform the source code and return the resulting string.
fn transform_to_string_with_options(source: &str, options: Options) -> Result<String, ParseError> {
    transform_to_string_with_options_timed(source, options).map(|(output, _)| output)
}

fn transform_to_string_with_options_timed(
    source: &str,
    options: Options,
) -> Result<(String, TransformTimings), ParseError> {
    if contains_surrogate_escape(source) {
        return Ok((
            source.to_string(),
            TransformTimings {
                parse: Duration::ZERO,
                rewrite: Duration::ZERO,
                ensure_import: Duration::ZERO,
                emit: Duration::ZERO,
                total: Duration::ZERO,
            },
        ));
    }
    let (module, mut timings) = transform_str_to_ruff_with_options_timed(source, options)?;
    let emit_start = Instant::now();
    let output = ruff_ast_to_string(&module.body);
    timings.emit = emit_start.elapsed();
    timings.total += timings.emit;
    Ok((output, timings))
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

pub fn transform_to_string_with_timing(
    source: &str,
    ensure: bool,
) -> Result<(String, TransformTimings), ParseError> {
    transform_to_string_with_options_timed(
        source,
        Options {
            inject_import: ensure,
            ..Options::default()
        },
    )
}

fn strip_generated_passes(stmts: &mut Vec<Stmt>) {
    struct StripGeneratedPasses;

    impl Transformer for StripGeneratedPasses {
        fn visit_body(&mut self, body: &mut Vec<Stmt>) {
            crate::body_transform::walk_body(self, body);
            if body.len() > 1 {
                body.retain(|stmt| !matches!(stmt, Stmt::Pass(_)));

                if body.is_empty() {
                    body.extend(crate::py_stmt!("pass"));
                }
            }
        }
    }

    let mut stripper = StripGeneratedPasses;
    stripper.visit_body(stmts);
}

pub fn transform_to_string_without_attribute_lowering(
    source: &str,
    ensure: bool,
) -> Result<String, ParseError> {
    transform_to_string_without_attribute_lowering_with_timing(source, ensure)
        .map(|(output, _)| output)
}

pub fn transform_to_string_without_attribute_lowering_with_timing(
    source: &str,
    ensure: bool,
) -> Result<(String, TransformTimings), ParseError> {
    transform_to_string_with_options_timed(
        source,
        Options {
            inject_import: ensure,
            lower_attributes: false,
            ..Options::default()
        },
    )
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
    transform_str_to_ruff_with_options_timed(source, options).map(|(module, _)| module)
}

pub fn transform_str_to_ruff_with_options_timed(
    source: &str,
    options: Options,
) -> Result<(ModModule, TransformTimings), ParseError> {
    let total_start = Instant::now();
    let parse_start = Instant::now();
    let mut module = parse_module(source)?.into_syntax();
    let parse = parse_start.elapsed();

    let rewrite_start = Instant::now();
    apply_transforms(&mut module, options, source);
    let rewrite = rewrite_start.elapsed();

    if options.lower_attributes {
        let _ = min_ast::Module::from(module.clone());
    }

    if options.cleanup_dp_globals {
        module.body.extend(crate::py_stmt!(
            r#"
for _dp_name in list(globals()):
    if _dp_name.startswith("_dp_"):
        del globals()[_dp_name]
if "_dp_name" in globals():
    del globals()["_dp_name"]
"#
        ));
    }

    let ensure_import = if options.inject_import {
        let ensure_start = Instant::now();
        ensure_import::ensure_import(&mut module);
        ensure_start.elapsed()
    } else {
        Duration::ZERO
    };

    let timings = TransformTimings {
        parse,
        rewrite,
        ensure_import,
        emit: Duration::ZERO,
        total: total_start.elapsed(),
    };

    Ok((module, timings))
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
        assert!(result.contains("setattr"));
        assert!(result.contains("import __dp__"));
    }
}
