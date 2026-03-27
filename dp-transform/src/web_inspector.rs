use crate::{lower_python_to_blockpy_recorded, render_inspector_payload};
use wasm_bindgen::JsValue;

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn transform(source: &str) -> Result<String, JsValue> {
    let result = lower_python_to_blockpy_recorded(source)
        .map_err(|e| JsValue::from_str(e.to_string().as_str()))?;
    Ok(result
        .pass_tracker
        .pass_ast_to_ast()
        .map(|module| crate::ruff_ast_to_string(&module.body))
        .unwrap_or_else(|| source.to_string()))
}

#[wasm_bindgen::prelude::wasm_bindgen]
pub fn inspect_pipeline(source: &str) -> Result<String, JsValue> {
    let transformed = lower_python_to_blockpy_recorded(source)
        .map_err(|e| JsValue::from_str(e.to_string().as_str()))?;
    Ok(render_inspector_payload(source, &transformed))
}
