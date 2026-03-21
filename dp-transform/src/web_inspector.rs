use crate::block_py::{
    pretty as blockpy_pretty, BlockPyFunction, BlockPyFunctionKind, BlockPyModule, BlockPyTerm,
    CoreBlockPyExprWithoutAwaitOrYield,
};
use crate::passes::BbBlockPyPass;
use crate::{transform_str_to_ruff_with_options, LoweringResult, Options};
use cranelift_codegen::ir::{self, condcodes::IntCC, types, AbiParam, InstBuilder, UserFuncName};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use js_sys::{Array, Object, Reflect};
use serde_json::{json, Value};
use std::collections::HashMap;
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

fn bb_module_to_json(module: &BlockPyModule<BbBlockPyPass>) -> Value {
    let functions = module
        .callable_defs
        .iter()
        .map(|function| {
            let blocks = function
                .blocks
                .iter()
                .map(|block| {
                    let ops_text = blockpy_pretty::bb_stmts_text(&block.body)
                        .trim()
                        .to_string();
                    let successors = bb_term_successors(&block.term)
                        .into_iter()
                        .map(|(target, edge_kind)| {
                            json!({
                                "target": target,
                                "kind": edge_kind,
                            })
                        })
                        .collect::<Vec<_>>();
                    json!({
                        "label": block.label.as_str(),
                        "params": block.param_name_vec(),
                        "opsText": ops_text,
                        "termKind": bb_term_kind(&block.term),
                        "termText": blockpy_pretty::bb_term_text(&block.term),
                        "successors": successors,
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "functionId": function.function_id.0,
                "bindName": function.names.bind_name,
                "displayName": function.names.display_name,
                "qualname": function.names.qualname,
                "kind": bb_function_kind_to_json(function.lowered_kind()),
                "entry": function.entry_block().label_str(),
                "paramNames": function.params.names(),
                "entryLiveins": function.entry_liveins(),
                "localCellSlots": function.local_cell_slots(),
                "blocks": blocks,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "moduleInit": module
            .callable_defs
            .iter()
            .any(|function| function.names.bind_name == "_dp_module_init")
            .then_some("_dp_module_init"),
        "functions": functions,
    })
}

fn bb_function_kind_to_json(kind: &BlockPyFunctionKind) -> Value {
    match kind {
        BlockPyFunctionKind::Function => json!({"kind": "function"}),
        BlockPyFunctionKind::Coroutine => {
            json!({"kind": "coroutine"})
        }
        BlockPyFunctionKind::Generator => {
            json!({"kind": "generator"})
        }
        BlockPyFunctionKind::AsyncGenerator => {
            json!({"kind": "async_generator"})
        }
    }
}

fn bb_term_kind(term: &BlockPyTerm<CoreBlockPyExprWithoutAwaitOrYield>) -> &'static str {
    match term {
        BlockPyTerm::Jump(_) => "jump",
        BlockPyTerm::IfTerm(_) => "br_if",
        BlockPyTerm::BranchTable(_) => "br_table",
        BlockPyTerm::Raise(_) => "raise",
        BlockPyTerm::Return(_) => "return",
        BlockPyTerm::TryJump(_) => "try_jump",
    }
}

fn bb_term_successors(
    term: &BlockPyTerm<CoreBlockPyExprWithoutAwaitOrYield>,
) -> Vec<(&str, &'static str)> {
    match term {
        BlockPyTerm::Jump(label) => vec![(label.as_str(), "jump")],
        BlockPyTerm::IfTerm(if_term) => vec![
            (if_term.then_label.as_str(), "branch_then"),
            (if_term.else_label.as_str(), "branch_else"),
        ],
        BlockPyTerm::BranchTable(branch) => {
            let mut out = branch
                .targets
                .iter()
                .map(|label| (label.as_str(), "table_target"))
                .collect::<Vec<_>>();
            out.push((branch.default_label.as_str(), "table_default"));
            out
        }
        BlockPyTerm::Raise(_) => Vec::new(),
        BlockPyTerm::Return(_) => Vec::new(),
        BlockPyTerm::TryJump(_) => Vec::new(),
    }
}

fn sanitize_clif_testcase_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "_dp_fn".to_string()
    } else {
        out
    }
}

fn clif_target_comment(
    label: &str,
    label_to_index: &HashMap<crate::block_py::BlockPyLabel, usize>,
    label_to_params: &HashMap<crate::block_py::BlockPyLabel, Vec<String>>,
) -> String {
    let target = label_to_index
        .get(label)
        .map(|index| format!("block{index}"))
        .unwrap_or_else(|| format!("%{label}"));
    let args = label_to_params.get(label).cloned().unwrap_or_default();
    if args.is_empty() {
        target
    } else {
        format!(
            "{target}({})",
            args.iter()
                .map(|name| format!("%{name}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn clif_term_comment(
    term: &BlockPyTerm<CoreBlockPyExprWithoutAwaitOrYield>,
    label_to_index: &HashMap<crate::block_py::BlockPyLabel, usize>,
    label_to_params: &HashMap<crate::block_py::BlockPyLabel, Vec<String>>,
) -> String {
    match term {
        BlockPyTerm::Jump(label) => {
            format!(
                "jump {}",
                clif_target_comment(label.as_str(), label_to_index, label_to_params)
            )
        }
        BlockPyTerm::IfTerm(if_term) => format!(
            "brif {}, {}, {}",
            blockpy_pretty::bb_expr_text(&if_term.test),
            clif_target_comment(if_term.then_label.as_str(), label_to_index, label_to_params),
            clif_target_comment(if_term.else_label.as_str(), label_to_index, label_to_params),
        ),
        BlockPyTerm::BranchTable(branch) => {
            let targets = branch
                .targets
                .iter()
                .map(|label| clif_target_comment(label.as_str(), label_to_index, label_to_params))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "br_table {}, [{}], {}",
                blockpy_pretty::bb_expr_text(&branch.index),
                targets,
                clif_target_comment(
                    branch.default_label.as_str(),
                    label_to_index,
                    label_to_params,
                ),
            )
        }
        BlockPyTerm::Raise(raise_stmt) => blockpy_pretty::bb_raise_text(raise_stmt),
        BlockPyTerm::Return(value) => {
            let value = value
                .as_ref()
                .map(blockpy_pretty::bb_expr_text)
                .unwrap_or_else(|| "None".to_string());
            format!("return {value}")
        }
        BlockPyTerm::TryJump(_) => "try_jump".to_string(),
    }
}

fn clif_target_args_for_block(
    builder: &mut FunctionBuilder<'_>,
    target_label: &str,
    current_values: &HashMap<String, ir::Value>,
    label_to_params: &HashMap<crate::block_py::BlockPyLabel, Vec<String>>,
) -> Vec<ir::BlockArg> {
    let mut args = Vec::new();
    if let Some(target_params) = label_to_params.get(target_label) {
        for name in target_params {
            let value = current_values
                .get(name)
                .copied()
                .unwrap_or_else(|| builder.ins().iconst(types::I64, 0));
            args.push(ir::BlockArg::Value(value));
        }
    }
    args
}

fn render_cranelift_function_from_bb(
    function: &BlockPyFunction<BbBlockPyPass>,
) -> Result<String, String> {
    if function.blocks.is_empty() {
        return Err("function has no blocks".to_string());
    }
    let entry_block = function.entry_block();

    let mut func = ir::Function::new();
    func.name = UserFuncName::testcase(sanitize_clif_testcase_name(
        function.names.qualname.as_str(),
    ));
    for _ in 0..entry_block.params.len() {
        func.signature.params.push(AbiParam::new(types::I64));
    }
    func.signature.returns.push(AbiParam::new(types::I64));

    let mut ctx = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut func, &mut ctx);

    let mut label_to_block = HashMap::new();
    let mut label_to_params = HashMap::new();
    for block in &function.blocks {
        label_to_block.insert(block.label.clone(), builder.create_block());
        label_to_params.insert(block.label.clone(), block.param_name_vec());
    }

    for (index, block) in function.blocks.iter().enumerate() {
        let clif_block = *label_to_block
            .get(block.label.as_str())
            .expect("block label must exist");
        if index == 0 {
            builder.append_block_params_for_function_params(clif_block);
            let existing = builder.block_params(clif_block).len();
            for _ in existing..block.params.len() {
                builder.append_block_param(clif_block, types::I64);
            }
        } else {
            for _ in &block.params {
                builder.append_block_param(clif_block, types::I64);
            }
        }
    }

    for block in &function.blocks {
        let clif_block = *label_to_block
            .get(block.label.as_str())
            .expect("block label must exist");
        builder.switch_to_block(clif_block);

        let mut current_values = HashMap::new();
        for (name, value) in block
            .param_names()
            .zip(builder.block_params(clif_block).iter().copied())
        {
            current_values.insert(name.to_string(), value);
        }

        // Preserve a stable one-op-per-source-op shape for web visualization.
        for _ in &block.body {
            let _ = builder.ins().iconst(types::I64, 0);
        }

        match &block.term {
            BlockPyTerm::Jump(target_label) => {
                let target = *label_to_block
                    .get(target_label.as_str())
                    .ok_or_else(|| format!("missing jump target: {}", target_label.as_str()))?;
                let args = clif_target_args_for_block(
                    &mut builder,
                    target_label.as_str(),
                    &current_values,
                    &label_to_params,
                );
                builder.ins().jump(target, &args);
            }
            BlockPyTerm::IfTerm(if_term) => {
                let then_label = &if_term.then_label;
                let else_label = &if_term.else_label;
                let then_block = *label_to_block
                    .get(then_label.as_str())
                    .ok_or_else(|| format!("missing brif then target: {then_label}"))?;
                let else_block = *label_to_block
                    .get(else_label.as_str())
                    .ok_or_else(|| format!("missing brif else target: {else_label}"))?;
                let cond_source = builder.ins().iconst(types::I64, 1);
                let cond = builder.ins().icmp_imm(IntCC::NotEqual, cond_source, 0);
                let then_args = clif_target_args_for_block(
                    &mut builder,
                    then_label.as_str(),
                    &current_values,
                    &label_to_params,
                );
                let else_args = clif_target_args_for_block(
                    &mut builder,
                    else_label.as_str(),
                    &current_values,
                    &label_to_params,
                );
                builder
                    .ins()
                    .brif(cond, then_block, &then_args, else_block, &else_args);
            }
            BlockPyTerm::BranchTable(branch) => {
                let default_label = &branch.default_label;
                // Web-view CLIF uses a valid fallback jump to keep the rendered IR
                // parseable/printable while preserving branch targets in comments.
                let default_block = *label_to_block
                    .get(default_label.as_str())
                    .ok_or_else(|| format!("missing br_table default target: {default_label}"))?;
                let args = clif_target_args_for_block(
                    &mut builder,
                    default_label.as_str(),
                    &current_values,
                    &label_to_params,
                );
                builder.ins().jump(default_block, &args);
            }
            BlockPyTerm::Raise(_) | BlockPyTerm::Return(_) => {
                let ret_value = builder.ins().iconst(types::I64, 0);
                builder.ins().return_(&[ret_value]);
            }
            BlockPyTerm::TryJump(_) => {
                return Err("TryJump is not allowed in BbTerm".to_string());
            }
        }
    }

    builder.seal_all_blocks();
    builder.finalize();
    Ok(func.display().to_string())
}

fn bb_module_to_clif(module: &BlockPyModule<BbBlockPyPass>) -> String {
    if module.callable_defs.is_empty() {
        return "; no basic-block functions emitted".to_string();
    }

    let mut out = String::new();
    out.push_str("; ---- CLIF declarations ----\n");
    out.push_str("type pyobj = i64\n");
    out.push_str("type termkind = i64\n");
    out.push('\n');
    out.push_str("; runtime helper declarations\n");
    out.push_str("decl @dp_jit_incref(pyobj)\n");
    out.push_str("decl @dp_jit_decref(pyobj)\n");
    out.push_str("decl @PyObject_CallFunctionObjArgs(pyobj, pyobj, pyobj, pyobj) -> pyobj\n");
    out.push_str("decl @PyObject_Call(pyobj, pyobj, pyobj) -> pyobj\n");
    out.push_str("decl @PyObject_GetAttr(pyobj, pyobj) -> pyobj\n");
    out.push_str("decl @PyObject_SetAttr(pyobj, pyobj, pyobj) -> i32\n");
    out.push_str("decl @PyObject_GetItem(pyobj, pyobj) -> pyobj\n");
    out.push_str("decl @PyObject_SetItem(pyobj, pyobj, pyobj) -> i32\n");
    out.push_str("decl @dp_jit_term_kind(pyobj) -> termkind\n");
    out.push_str("decl @dp_jit_raise_from_exc(pyobj) -> i32\n");
    out.push_str("decl @dp_jit_term_invalid(pyobj) -> i32\n");
    out.push('\n');
    out.push_str("; generated function declarations\n");
    for function in &module.callable_defs {
        let params = function
            .params
            .names()
            .into_iter()
            .map(|name| format!("%{name}: pyobj"))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "decl %{}({params}) -> pyobj ; bind={}\n",
            function.names.qualname, function.names.bind_name,
        ));
    }
    if module
        .callable_defs
        .iter()
        .any(|function| function.names.bind_name == "_dp_module_init")
    {
        out.push_str("decl %_dp_module_init() -> pyobj ; module_init\n");
    }
    out.push('\n');

    out.push_str("; ---- rendered with Cranelift Function::display() ----\n");
    for function in &module.callable_defs {
        let mut label_to_index = HashMap::new();
        let mut label_to_params = HashMap::new();
        for (index, block) in function.blocks.iter().enumerate() {
            label_to_index.insert(block.label.clone(), index);
            label_to_params.insert(block.label.clone(), block.param_name_vec());
        }

        out.push_str(&format!(
            "; function {} (kind={:?}, bind={}, entry={})\n",
            function.names.qualname,
            function.lowered_kind(),
            function.names.bind_name,
            function.entry_block().label_str()
        ));
        for block in &function.blocks {
            out.push_str(&format!(
                ";   {}({})\n",
                block.label,
                block
                    .param_names()
                    .map(|name| format!("%{name}: pyobj"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            if let Some(exc_target) = block.meta.exc_edge.as_ref().map(|edge| &edge.target) {
                out.push_str(&format!(
                    ";     exc_target={}\n",
                    clif_target_comment(exc_target, &label_to_index, &label_to_params)
                ));
            }
            if let Some(exc_name) = block.exception_param() {
                out.push_str(&format!(";     exc_name=%{exc_name}\n"));
            }
            let ops = blockpy_pretty::bb_stmts_text(&block.body);
            for line in ops.lines().filter(|line| !line.trim().is_empty()) {
                out.push_str(";     op: ");
                out.push_str(line.trim_end());
                out.push('\n');
            }
            out.push_str(";     term: ");
            out.push_str(&clif_term_comment(
                &block.term,
                &label_to_index,
                &label_to_params,
            ));
            out.push('\n');
        }

        match render_cranelift_function_from_bb(function) {
            Ok(rendered) => {
                out.push_str(&rendered);
                if !rendered.ends_with('\n') {
                    out.push('\n');
                }
            }
            Err(err) => {
                out.push_str(&format!(
                    "; failed to render function {} via Cranelift display: {err}\n",
                    function.names.qualname
                ));
            }
        }
        out.push('\n');
    }

    out
}
