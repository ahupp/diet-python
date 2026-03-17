use crate::basic_block::bb_ir;
use crate::{ruff_ast_to_string, transform_str_to_ruff_with_options, Options};
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
    let blockpy = transformed
        .get_pass::<crate::basic_block::LoweredBlockPyModuleBundle>("semantic_blockpy_materialized")
        .map(|bundle| {
            crate::basic_block::blockpy_module_to_string(
                &crate::basic_block::project_lowered_module_callable_defs(
                    bundle,
                    |lowered| -> &crate::basic_block::block_py::SemanticBlockPyCallableDef {
                        lowered
                    },
                ),
            )
        })
        .unwrap_or_else(|| "; no BlockPy module emitted".to_string());
    let bb_module = transformed
        .bb_module
        .as_ref()
        .ok_or_else(|| JsValue::from_str("expected BB module from lowering"))?;
    let bb_module_json = bb_module_to_json(&bb_module);
    let clif = Some(bb_module)
        .map(crate::basic_block::normalize_bb_module_for_codegen)
        .map(|module| bb_module_to_clif(&module));
    let lowering_ast = transformed
        .get_pass::<ruff_python_ast::StmtBody>("ast-to-ast")
        .map(ruff_ast_to_string)
        .unwrap_or_else(|| transformed.to_string());
    let core_blockpy = transformed
        .get_pass::<crate::basic_block::LoweredCoreBlockPyModuleBundle>("core_blockpy")
        .map(|bundle| {
            crate::basic_block::blockpy_module_to_string(
                &crate::basic_block::project_lowered_module_callable_defs(
                    bundle,
                    |lowered| -> &crate::basic_block::block_py::CoreBlockPyCallableDef { lowered },
                ),
            )
        })
        .unwrap_or_default();

    let payload = json!({
        "phase1": lowering_ast,
        "blockpy": blockpy,
        "coreBlockPy": core_blockpy,
        "bbRaw": transformed
            .get_pass::<ruff_python_ast::StmtBody>("ast-to-ast")
            .map(ruff_ast_to_string)
            .unwrap_or_else(|| transformed.to_string()),
        "rewrittenAstFinal": transformed.to_string(),
        "bbModule": bb_module_json,
        "clif": clif,
    });
    Ok(payload.to_string())
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

fn bb_module_to_json(module: &bb_ir::BbModule) -> Value {
    let functions = module
        .callable_defs
        .iter()
        .map(|function| {
            let blocks = function
                .blocks
                .iter()
                .map(|block| {
                    let ops_text = bb_ir::bb_stmts_text(&block.body).trim().to_string();
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
                        "label": block.label,
                        "params": block.meta.params,
                        "opsText": ops_text,
                        "termKind": bb_term_kind(&block.term),
                        "termText": bb_term_text(&block.term),
                        "successors": successors,
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "functionId": function.function_id.0,
                "bindName": function.bind_name,
                "displayName": function.display_name,
                "qualname": function.qualname,
                "bindingTarget": bb_binding_target_name(function.binding_target()),
                "kind": bb_function_kind_to_json(&function.kind),
                "entry": function.entry_label(),
                "paramNames": function.params,
                "entryLiveins": function.entry_liveins,
                "localCellSlots": function.local_cell_slots(),
                "blocks": blocks,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "moduleInit": module.module_init,
        "functions": functions,
    })
}

fn bb_binding_target_name(target: crate::basic_block::lowered_ir::BindingTarget) -> &'static str {
    match target {
        crate::basic_block::lowered_ir::BindingTarget::Local => "local",
        crate::basic_block::lowered_ir::BindingTarget::ModuleGlobal => "module_global",
        crate::basic_block::lowered_ir::BindingTarget::ClassNamespace => "class_namespace",
    }
}

fn bb_function_kind_to_json(kind: &crate::basic_block::lowered_ir::LoweredFunctionKind) -> Value {
    match kind {
        crate::basic_block::lowered_ir::LoweredFunctionKind::Function => {
            json!({"kind": "function"})
        }
        crate::basic_block::lowered_ir::LoweredFunctionKind::Generator {
            closure_state,
            resume_label,
            target_labels,
            resume_pcs,
        } => json!({
            "kind": "generator",
            "closureState": closure_state,
            "resumeLabel": resume_label,
            "targetLabels": target_labels,
            "resumePcs": resume_pcs,
        }),
        crate::basic_block::lowered_ir::LoweredFunctionKind::AsyncGenerator {
            closure_state,
            resume_label,
            target_labels,
            resume_pcs,
        } => json!({
            "kind": "async_generator",
            "closureState": closure_state,
            "resumeLabel": resume_label,
            "targetLabels": target_labels,
            "resumePcs": resume_pcs,
        }),
    }
}

fn bb_term_kind(term: &bb_ir::BbTerm) -> &'static str {
    match term {
        bb_ir::BbTerm::Jump(_) => "jump",
        bb_ir::BbTerm::BrIf { .. } => "br_if",
        bb_ir::BbTerm::BrTable { .. } => "br_table",
        bb_ir::BbTerm::Raise { .. } => "raise",
        bb_ir::BbTerm::Ret(_) => "return",
    }
}

fn bb_term_text(term: &bb_ir::BbTerm) -> String {
    match term {
        bb_ir::BbTerm::Jump(label) => format!("jump {label}"),
        bb_ir::BbTerm::BrIf {
            test,
            then_label,
            else_label,
        } => {
            let test = expr_to_one_line(test);
            format!("if {test} then {then_label} else {else_label}")
        }
        bb_ir::BbTerm::BrTable {
            index,
            targets,
            default_label,
        } => {
            let index = expr_to_one_line(index);
            format!(
                "br_table index={index} targets=[{}] default={default_label}",
                targets.join(", ")
            )
        }
        bb_ir::BbTerm::Raise { exc, cause } => {
            let exc = exc
                .as_ref()
                .map(expr_to_one_line)
                .unwrap_or_else(|| "None".to_string());
            let cause = cause
                .as_ref()
                .map(expr_to_one_line)
                .unwrap_or_else(|| "None".to_string());
            format!("raise exc={exc} cause={cause}")
        }
        bb_ir::BbTerm::Ret(value) => {
            let value = value
                .as_ref()
                .map(expr_to_one_line)
                .unwrap_or_else(|| "None".to_string());
            format!("return {value}")
        }
    }
}

fn bb_term_successors(term: &bb_ir::BbTerm) -> Vec<(&str, &'static str)> {
    match term {
        bb_ir::BbTerm::Jump(label) => vec![(label.as_str(), "jump")],
        bb_ir::BbTerm::BrIf {
            then_label,
            else_label,
            ..
        } => vec![
            (then_label.as_str(), "branch_then"),
            (else_label.as_str(), "branch_else"),
        ],
        bb_ir::BbTerm::BrTable {
            targets,
            default_label,
            ..
        } => {
            let mut out = targets
                .iter()
                .map(|label| (label.as_str(), "table_target"))
                .collect::<Vec<_>>();
            out.push((default_label.as_str(), "table_default"));
            out
        }
        bb_ir::BbTerm::Raise { .. } => Vec::new(),
        bb_ir::BbTerm::Ret(_) => Vec::new(),
    }
}

fn expr_to_one_line(
    expr: &crate::basic_block::block_py::CoreBlockPyExprWithoutAwaitOrYield,
) -> String {
    bb_ir::bb_expr_text(expr)
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
    label_to_index: &HashMap<String, usize>,
    label_to_params: &HashMap<String, Vec<String>>,
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
    term: &bb_ir::BbTerm,
    label_to_index: &HashMap<String, usize>,
    label_to_params: &HashMap<String, Vec<String>>,
) -> String {
    match term {
        bb_ir::BbTerm::Jump(label) => {
            format!(
                "jump {}",
                clif_target_comment(label.as_str(), label_to_index, label_to_params)
            )
        }
        bb_ir::BbTerm::BrIf {
            test,
            then_label,
            else_label,
        } => format!(
            "brif {}, {}, {}",
            expr_to_one_line(test),
            clif_target_comment(then_label.as_str(), label_to_index, label_to_params),
            clif_target_comment(else_label.as_str(), label_to_index, label_to_params),
        ),
        bb_ir::BbTerm::BrTable {
            index,
            targets,
            default_label,
        } => {
            let targets = targets
                .iter()
                .map(|label| clif_target_comment(label.as_str(), label_to_index, label_to_params))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "br_table {}, [{}], {}",
                expr_to_one_line(index),
                targets,
                clif_target_comment(default_label.as_str(), label_to_index, label_to_params),
            )
        }
        bb_ir::BbTerm::Raise { exc, cause } => {
            let exc = exc
                .as_ref()
                .map(expr_to_one_line)
                .unwrap_or_else(|| "None".to_string());
            let cause = cause
                .as_ref()
                .map(expr_to_one_line)
                .unwrap_or_else(|| "None".to_string());
            format!("raise exc={exc} cause={cause}")
        }
        bb_ir::BbTerm::Ret(value) => {
            let value = value
                .as_ref()
                .map(expr_to_one_line)
                .unwrap_or_else(|| "None".to_string());
            format!("return {value}")
        }
    }
}

fn clif_target_args_for_block(
    builder: &mut FunctionBuilder<'_>,
    target_label: &str,
    current_values: &HashMap<String, ir::Value>,
    label_to_params: &HashMap<String, Vec<String>>,
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

fn render_cranelift_function_from_bb(function: &bb_ir::BbFunction) -> Result<String, String> {
    if function.blocks.is_empty() {
        return Err("function has no blocks".to_string());
    }
    let entry_label = function.entry_label();
    let entry_block = function
        .blocks
        .iter()
        .find(|block| block.label == entry_label)
        .ok_or_else(|| format!("missing entry block: {entry_label}"))?;

    let mut func = ir::Function::new();
    func.name = UserFuncName::testcase(sanitize_clif_testcase_name(function.qualname.as_str()));
    for _ in 0..entry_block.meta.params.len() {
        func.signature.params.push(AbiParam::new(types::I64));
    }
    func.signature.returns.push(AbiParam::new(types::I64));

    let mut ctx = FunctionBuilderContext::new();
    let mut builder = FunctionBuilder::new(&mut func, &mut ctx);

    let mut label_to_block = HashMap::new();
    let mut label_to_params = HashMap::new();
    for block in &function.blocks {
        label_to_block.insert(block.label.clone(), builder.create_block());
        label_to_params.insert(block.label.clone(), block.meta.params.clone());
    }

    for block in &function.blocks {
        let clif_block = *label_to_block
            .get(block.label.as_str())
            .expect("block label must exist");
        if block.label == entry_label {
            builder.append_block_params_for_function_params(clif_block);
            let existing = builder.block_params(clif_block).len();
            for _ in existing..block.meta.params.len() {
                builder.append_block_param(clif_block, types::I64);
            }
        } else {
            for _ in &block.meta.params {
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
            .meta
            .params
            .iter()
            .zip(builder.block_params(clif_block).iter().copied())
        {
            current_values.insert(name.clone(), value);
        }

        // Preserve a stable one-op-per-source-op shape for web visualization.
        for _ in &block.body {
            let _ = builder.ins().iconst(types::I64, 0);
        }

        match &block.term {
            bb_ir::BbTerm::Jump(target_label) => {
                let target = *label_to_block
                    .get(target_label.as_str())
                    .ok_or_else(|| format!("missing jump target: {target_label}"))?;
                let args = clif_target_args_for_block(
                    &mut builder,
                    target_label.as_str(),
                    &current_values,
                    &label_to_params,
                );
                builder.ins().jump(target, &args);
            }
            bb_ir::BbTerm::BrIf {
                then_label,
                else_label,
                ..
            } => {
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
            bb_ir::BbTerm::BrTable { default_label, .. } => {
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
            bb_ir::BbTerm::Raise { .. } | bb_ir::BbTerm::Ret(_) => {
                let ret_value = builder.ins().iconst(types::I64, 0);
                builder.ins().return_(&[ret_value]);
            }
        }
    }

    builder.seal_all_blocks();
    builder.finalize();
    Ok(func.display().to_string())
}

fn bb_module_to_clif(module: &bb_ir::BbModule) -> String {
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
            .iter()
            .map(|name| format!("%{name}: pyobj"))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "decl %{}({params}) -> pyobj ; bind={} target={:?}\n",
            function.qualname,
            function.bind_name,
            function.binding_target()
        ));
    }
    if let Some(module_init) = module.module_init.as_ref() {
        out.push_str(&format!("decl %{module_init}() -> pyobj ; module_init\n"));
    }
    out.push('\n');

    out.push_str("; ---- rendered with Cranelift Function::display() ----\n");
    for function in &module.callable_defs {
        let mut label_to_index = HashMap::new();
        let mut label_to_params = HashMap::new();
        for (index, block) in function.blocks.iter().enumerate() {
            label_to_index.insert(block.label.clone(), index);
            label_to_params.insert(block.label.clone(), block.meta.params.clone());
        }

        out.push_str(&format!(
            "; function {} (kind={:?}, bind={}, target={:?}, entry={})\n",
            function.qualname,
            function.kind,
            function.bind_name,
            function.binding_target(),
            function.entry_label()
        ));
        for block in &function.blocks {
            out.push_str(&format!(
                ";   {}({})\n",
                block.label,
                block
                    .meta
                    .params
                    .iter()
                    .map(|name| format!("%{name}: pyobj"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            if let Some(exc_target) = block.meta.exc_target_label.as_ref() {
                out.push_str(&format!(
                    ";     exc_target={}\n",
                    clif_target_comment(exc_target, &label_to_index, &label_to_params)
                ));
            }
            if let Some(exc_name) = block.meta.exc_name.as_ref() {
                out.push_str(&format!(";     exc_name=%{exc_name}\n"));
            }
            let ops = bb_ir::bb_stmts_text(&block.body);
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
                    function.qualname
                ));
            }
        }
        out.push('\n');
    }

    out
}
