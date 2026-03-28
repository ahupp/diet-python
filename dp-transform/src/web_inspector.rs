use crate::{transform_str_to_ruff, LoweringResult};
use serde_json::{json, Value};
use wasm_bindgen::JsValue;

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn transform(source: &str) -> Result<String, JsValue> {
    let result =
        transform_str_to_ruff(source).map_err(|e| JsValue::from_str(e.to_string().as_str()))?;
    Ok(result.to_string())
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn inspect_pipeline(source: &str) -> Result<String, JsValue> {
    let transformed =
        transform_str_to_ruff(source).map_err(|e| JsValue::from_str(e.to_string().as_str()))?;
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
    for name in transformed.pass_tracker.pass_names() {
        let text = transformed
            .pass_tracker
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
