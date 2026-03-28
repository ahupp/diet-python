use crate::{transform_str_to_ruff_with_options, LoweringResult, Options};
use serde_json::{json, Value};
use wasm_bindgen::JsValue;

pub fn transform(source: &str) -> Result<String, JsValue> {
    let options = Options::default();
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
