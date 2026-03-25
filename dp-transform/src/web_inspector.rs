use crate::{transform_str_to_ruff_with_options, LoweringResult, Options};
use js_sys::{Array, Object, Reflect};
use serde_json::{json, Value};
use wasm_bindgen::JsValue;

#[derive(Clone, Copy)]
enum TransformKind {
    LowerAttributes,
}

struct TransformToggle {
    id: &'static str,
    label: &'static str,
    default_enabled: bool,
    kind: TransformKind,
}

const TRANSFORM_TOGGLES: &[TransformToggle] = &[TransformToggle {
    id: "lower_attributes",
    label: "Rewrite attribute access",
    default_enabled: true,
    kind: TransformKind::LowerAttributes,
}];
pub fn transform(source: &str) -> Result<String, JsValue> {
    let options = Options::default();
    let result = transform_str_to_ruff_with_options(source, options)
        .map_err(|e| JsValue::from_str(e.to_string().as_str()))?;
    Ok(result.to_string())
}

pub fn transform_selected(source: &str, transforms: Array) -> Result<String, JsValue> {
    let options = wasm_options_from_selected(&transforms);
    let result = transform_str_to_ruff_with_options(source, options)
        .map_err(|e| JsValue::from_str(e.to_string().as_str()))?;
    Ok(result.to_string())
}

pub fn inspect_pipeline(source: &str) -> Result<String, JsValue> {
    let transformed = transform_str_to_ruff_with_options(source, Options::default())
        .map_err(|e| JsValue::from_str(e.to_string().as_str()))?;
    let payload = json!({
        "steps": pipeline_steps(source, &transformed),
    });
    Ok(payload.to_string())
}

fn pipeline_steps(source: &str, transformed: &LoweringResult) -> Vec<Value> {
    let mut steps = vec![json!({
        "key": "input_source",
        "label": "input source",
        "text": source,
    })];
    for name in transformed.pass_names() {
        let text = transformed
            .render_pass_text(name)
            .unwrap_or_else(|| format!("; no text renderer for pass {name}"));
        steps.push(json!({
            "key": name,
            "label": name,
            "text": text,
        }));
    }
    steps
}

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

fn wasm_options_from_selected(transforms: &Array) -> Options {
    let selected: Vec<String> = transforms
        .iter()
        .filter_map(|value| value.as_string())
        .collect();
    let mut options = Options::default();
    for transform in TRANSFORM_TOGGLES {
        let enabled = selected.iter().any(|name| name == transform.id);
        match transform.kind {
            TransformKind::LowerAttributes => options.lower_attributes = enabled,
        }
    }
    options
}
