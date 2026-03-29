use crate::block_py::BlockPyFunction;
use crate::passes::CodegenBlockPyPass;
use crate::LoweringResult;
use serde_json::{json, Value};

fn inspector_function_payload(function: &BlockPyFunction<CodegenBlockPyPass>) -> Value {
    json!({
        "functionId": function.function_id.0,
        "qualname": function.names.qualname,
        "displayName": function.names.display_name,
        "bindName": function.names.bind_name,
        "kind": format!("{:?}", function.kind).to_lowercase(),
        "entryLabel": function.entry_block().label_str(),
    })
}

fn inspector_functions_payload(
    module: &crate::block_py::BlockPyModule<CodegenBlockPyPass>,
) -> Vec<Value> {
    module
        .callable_defs
        .iter()
        .map(inspector_function_payload)
        .collect::<Vec<_>>()
}

pub fn render_inspector_payload(source: &str, output: &LoweringResult) -> String {
    let mut steps = vec![json!({
        "key": "input_source",
        "label": "input source",
        "text": source,
    })];
    for name in output.pass_tracker.pass_names() {
        let text = output
            .pass_tracker
            .render_pass_text(name)
            .unwrap_or_else(|| format!("; no text renderer for pass {name}"));
        steps.push(json!({
            "key": name,
            "label": name,
            "text": text,
        }));
    }
    json!({
        "steps": steps,
        "functions": inspector_functions_payload(&output.codegen_module),
    })
    .to_string()
}
