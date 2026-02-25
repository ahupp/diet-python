#[cfg(target_arch = "wasm32")]
use cranelift_codegen::ir::{self, AbiParam, InstBuilder, UserFuncName, condcodes::IntCC, types};
#[cfg(target_arch = "wasm32")]
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
#[cfg(target_arch = "wasm32")]
use js_sys::{Array, Object, Reflect};
use ruff_python_ast::{self as ast, Expr, Stmt, StmtBody};
use ruff_python_codegen::{Generator, Indentation};
use ruff_python_parser::parse_module;
pub use ruff_python_parser::ParseError;
use ruff_source_file::LineEnding;
use ruff_text_size::TextRange;
#[cfg(target_arch = "wasm32")]
use serde_json::{json, Value};
#[cfg(target_arch = "wasm32")]
use std::collections::HashMap;
use std::sync::Once;
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

pub mod basic_block;
pub mod ensure_import;
pub mod fixture;
pub mod min_ast;
mod namegen;
pub mod scope_aware_transformer;
pub mod side_by_side;
mod template;
#[cfg(test)]
mod test_util;
mod transform;
pub(crate) mod transformer;

use crate::basic_block::bb_ir;
use crate::transform::driver::rewrite_module;
pub use crate::transform::scope::{analyze_module_scope, Scope};
use transform::context::Context;
pub use transform::Options;

#[derive(Debug, Clone, Copy)]
pub struct TransformTimings {
    pub parse_time: Duration,
    pub rewrite_time: Duration,
    pub total_time: Duration,
}

static INIT_LOGGER: Once = Once::new();

fn timing_start() -> Option<Instant> {
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Some(Instant::now())
    }
}

fn timing_elapsed(start: Option<Instant>) -> Duration {
    start.map_or(Duration::ZERO, |instant| instant.elapsed())
}

pub fn init_logging() {
    INIT_LOGGER.call_once(|| {
        let mut builder =
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(""));
        if cfg!(test) {
            builder.is_test(true);
        }
        let _ = builder.try_init();
    });
}

fn should_skip(source: &str) -> bool {
    source
        .lines()
        .next()
        .is_some_and(|line| line.contains("diet-python: disabled"))
}

pub struct LoweringResult {
    pub timings: TransformTimings,
    pub module: ruff_python_ast::ModModule,
    pub bb_module: Option<bb_ir::BbModule>,
    function_name_map: std::collections::HashMap<String, (String, String)>,
}

impl LoweringResult {
    pub fn to_string(&self) -> String {
        ruff_ast_to_string(&self.module.body)
    }

    pub fn into_min_ast(self) -> min_ast::Module {
        min_ast::Module::from_with_function_name_map(self.module, &self.function_name_map)
    }
}

/// Transform the source code and return the resulting Ruff AST.
pub fn transform_str_to_ruff_with_options(
    source: &str,
    options: Options,
) -> Result<LoweringResult, ParseError> {
    init_logging();
    namegen::reset_namegen_state();

    let total_start = timing_start();

    let parse_start = timing_start();
    let mut module = parse_module(source)?.into_syntax();
    let parse_time = timing_elapsed(parse_start);

    if should_skip(source) {
        return Ok(LoweringResult {
            timings: TransformTimings {
                parse_time: Duration::from_nanos(0),
                rewrite_time: Duration::from_nanos(0),
                total_time: Duration::from_nanos(0),
            },
            module,
            bb_module: None,
            function_name_map: std::collections::HashMap::new(),
        });
    }

    let ctx = Context::new(options, source);

    let rewrite_start = timing_start();

    let rewrite_result = rewrite_module(&ctx, &mut module.body);
    let rewrite_time = timing_elapsed(rewrite_start);

    let timings = TransformTimings {
        parse_time,
        rewrite_time,
        total_time: timing_elapsed(total_start),
    };

    Ok(LoweringResult {
        timings,
        module,
        bb_module: rewrite_result.bb_module,
        function_name_map: rewrite_result.function_name_map,
    })
}

pub fn transform_str_to_bb_ir_with_options(
    source: &str,
    options: Options,
) -> Result<Option<bb_ir::BbModule>, ParseError> {
    Ok(transform_str_to_ruff_with_options(source, options)?.bb_module)
}

pub trait ToRuffAst {
    fn to_ruff_ast(&self) -> Vec<Stmt>;
}

impl ToRuffAst for Expr {
    fn to_ruff_ast(&self) -> Vec<Stmt> {
        vec![Stmt::Expr(ast::StmtExpr {
            value: Box::new(self.clone()),
            range: TextRange::default(),
            node_index: ast::AtomicNodeIndex::default(),
        })]
    }
}

impl ToRuffAst for Stmt {
    fn to_ruff_ast(&self) -> Vec<Stmt> {
        vec![self.clone()]
    }
}

impl ToRuffAst for &Stmt {
    fn to_ruff_ast(&self) -> Vec<Stmt> {
        vec![self.to_owned().clone()]
    }
}

impl ToRuffAst for &Vec<Stmt> {
    fn to_ruff_ast(&self) -> Vec<Stmt> {
        self.to_vec()
    }
}

impl ToRuffAst for &Box<Stmt> {
    fn to_ruff_ast(&self) -> Vec<Stmt> {
        if let Some(body) = self.as_body() {
            body.iter().map(|stmt| stmt.as_ref().clone()).collect()
        } else {
            vec![self.as_ref().clone()]
        }
    }
}

impl ToRuffAst for &[Box<Stmt>] {
    fn to_ruff_ast(&self) -> Vec<Stmt> {
        self.iter().map(|stmt| stmt.as_ref().clone()).collect()
    }
}

impl ToRuffAst for &Vec<Box<Stmt>> {
    fn to_ruff_ast(&self) -> Vec<Stmt> {
        self.iter().map(|stmt| stmt.as_ref().clone()).collect()
    }
}

impl ToRuffAst for StmtBody {
    fn to_ruff_ast(&self) -> Vec<Stmt> {
        self.body.iter().map(|stmt| stmt.as_ref().clone()).collect()
    }
}

impl ToRuffAst for &StmtBody {
    fn to_ruff_ast(&self) -> Vec<Stmt> {
        self.body.iter().map(|stmt| stmt.as_ref().clone()).collect()
    }
}

impl ToRuffAst for &Expr {
    fn to_ruff_ast(&self) -> Vec<Stmt> {
        let expr = self.to_owned().clone();
        vec![Stmt::Expr(ast::StmtExpr {
            value: Box::new(expr),
            range: TextRange::default(),
            node_index: ast::AtomicNodeIndex::default(),
        })]
    }
}

impl ToRuffAst for &[Stmt] {
    fn to_ruff_ast(&self) -> Vec<Stmt> {
        self.to_vec()
    }
}

/// Convert a ruff AST ModModule to a pretty-printed string.
pub fn ruff_ast_to_string(module: impl ToRuffAst) -> String {
    let module = module.to_ruff_ast();
    // Use default stylist settings for pretty printing
    let indent = Indentation::new("    ".to_string());
    let mut output = String::new();
    for stmt in module {
        let gen = Generator::new(&indent, LineEnding::default());
        output.push_str(&gen.stmt(&stmt));
        output.push_str(LineEnding::default().as_str());
    }
    output
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn transform(source: &str) -> Result<String, JsValue> {
    let options = Options::default();
    let result = transform_str_to_ruff_with_options(source, options)
        .map_err(|e| JsValue::from_str(e.to_string().as_str()))?;
    Ok(result.to_string())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn transform_selected(source: &str, transforms: Array) -> Result<String, JsValue> {
    let options = wasm_options_from_selected(&transforms);
    let result = transform_str_to_ruff_with_options(source, options)
        .map_err(|e| JsValue::from_str(e.to_string().as_str()))?;
    Ok(result.to_string())
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn inspect_pipeline(source: &str) -> Result<String, JsValue> {
    let mut phase_one_options = Options::default();
    phase_one_options.emit_basic_blocks = false;

    let phase_one = transform_str_to_ruff_with_options(source, phase_one_options)
        .map_err(|e| JsValue::from_str(e.to_string().as_str()))?;
    let bb = transform_str_to_ruff_with_options(source, Options::default())
        .map_err(|e| JsValue::from_str(e.to_string().as_str()))?;
    let bb_module_json = bb
        .bb_module
        .as_ref()
        .map(bb_module_to_json)
        .unwrap_or(Value::Null);
    let clif = bb
        .bb_module
        .as_ref()
        .map(crate::basic_block::normalize_bb_module_for_codegen)
        .map(|module| bb_module_to_clif(&module))
        .unwrap_or_else(|| "; no basic-block module emitted".to_string());

    let payload = json!({
        "phase1": phase_one.to_string(),
        "bbRaw": bb.to_string(),
        "bbModule": bb_module_json,
        "clif": clif,
    });
    Ok(payload.to_string())
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

#[cfg(target_arch = "wasm32")]
fn bb_module_to_json(module: &bb_ir::BbModule) -> Value {
    let functions = module
        .functions
        .iter()
        .map(|function| {
            let blocks = function
                .blocks
                .iter()
                .map(|block| {
                    let ops_text = ruff_ast_to_string(&bb_ir::bb_ops_to_stmts(&block.ops))
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
                        "label": block.label,
                        "params": block.params,
                        "opsText": ops_text,
                        "termKind": bb_term_kind(&block.term),
                        "termText": bb_term_text(&block.term),
                        "successors": successors,
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "bindName": function.bind_name,
                "displayName": function.display_name,
                "qualname": function.qualname,
                "bindingTarget": bb_binding_target_name(function.binding_target),
                "kind": bb_function_kind_to_json(&function.kind),
                "entry": function.entry,
                "paramNames": function.param_names,
                "entryParams": function.entry_params,
                "localCellSlots": function.local_cell_slots,
                "blocks": blocks,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "moduleInit": module.module_init,
        "functions": functions,
    })
}

#[cfg(target_arch = "wasm32")]
fn bb_binding_target_name(target: bb_ir::BbBindingTarget) -> &'static str {
    match target {
        bb_ir::BbBindingTarget::Local => "local",
        bb_ir::BbBindingTarget::ModuleGlobal => "module_global",
        bb_ir::BbBindingTarget::ClassNamespace => "class_namespace",
    }
}

#[cfg(target_arch = "wasm32")]
fn bb_function_kind_to_json(kind: &bb_ir::BbFunctionKind) -> Value {
    match kind {
        bb_ir::BbFunctionKind::Function => json!({"kind": "function"}),
        bb_ir::BbFunctionKind::Coroutine => json!({"kind": "coroutine"}),
        bb_ir::BbFunctionKind::Generator {
            resume_label,
            target_labels,
            resume_pcs,
        } => json!({
            "kind": "generator",
            "resumeLabel": resume_label,
            "targetLabels": target_labels,
            "resumePcs": resume_pcs,
        }),
        bb_ir::BbFunctionKind::AsyncGenerator {
            resume_label,
            target_labels,
            resume_pcs,
        } => json!({
            "kind": "async_generator",
            "resumeLabel": resume_label,
            "targetLabels": target_labels,
            "resumePcs": resume_pcs,
        }),
    }
}

#[cfg(target_arch = "wasm32")]
fn bb_term_kind(term: &bb_ir::BbTerm) -> &'static str {
    match term {
        bb_ir::BbTerm::Jump(_) => "jump",
        bb_ir::BbTerm::BrIf { .. } => "br_if",
        bb_ir::BbTerm::BrTable { .. } => "br_table",
        bb_ir::BbTerm::Raise { .. } => "raise",
        bb_ir::BbTerm::TryJump { .. } => "try_jump",
        bb_ir::BbTerm::Ret(_) => "return",
    }
}

#[cfg(target_arch = "wasm32")]
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
        bb_ir::BbTerm::TryJump {
            body_label,
            except_label,
            finally_label,
            finally_fallthrough_label,
            ..
        } => format!(
            "try body={body_label} except={except_label} finally={} finally_fallthrough={}",
            finally_label.as_deref().unwrap_or("-"),
            finally_fallthrough_label.as_deref().unwrap_or("-"),
        ),
        bb_ir::BbTerm::Ret(value) => {
            let value = value
                .as_ref()
                .map(expr_to_one_line)
                .unwrap_or_else(|| "None".to_string());
            format!("return {value}")
        }
    }
}

#[cfg(target_arch = "wasm32")]
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
        bb_ir::BbTerm::TryJump {
            body_label,
            except_label,
            finally_label,
            finally_fallthrough_label,
            ..
        } => {
            let mut out = vec![
                (body_label.as_str(), "try_body"),
                (except_label.as_str(), "try_except"),
            ];
            if let Some(label) = finally_label {
                out.push((label.as_str(), "try_finally"));
            }
            if let Some(label) = finally_fallthrough_label {
                out.push((label.as_str(), "try_fallthrough"));
            }
            out
        }
        bb_ir::BbTerm::Ret(_) => Vec::new(),
    }
}

#[cfg(target_arch = "wasm32")]
fn expr_to_one_line(expr: &Expr) -> String {
    ruff_ast_to_string(expr)
        .lines()
        .next()
        .map(|line| line.trim().to_string())
        .unwrap_or_default()
}

#[cfg(target_arch = "wasm32")]
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

#[cfg(target_arch = "wasm32")]
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

#[cfg(target_arch = "wasm32")]
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
        bb_ir::BbTerm::TryJump {
            body_label,
            except_label,
            finally_label,
            finally_fallthrough_label,
            ..
        } => format!(
            "try_jump body={} except={} finally={} fallthrough={}",
            clif_target_comment(body_label.as_str(), label_to_index, label_to_params),
            clif_target_comment(except_label.as_str(), label_to_index, label_to_params),
            finally_label
                .as_ref()
                .map(|label| clif_target_comment(label.as_str(), label_to_index, label_to_params))
                .unwrap_or_else(|| "-".to_string()),
            finally_fallthrough_label
                .as_ref()
                .map(|label| clif_target_comment(label.as_str(), label_to_index, label_to_params))
                .unwrap_or_else(|| "-".to_string()),
        ),
        bb_ir::BbTerm::Ret(value) => {
            let value = value
                .as_ref()
                .map(expr_to_one_line)
                .unwrap_or_else(|| "None".to_string());
            format!("return {value}")
        }
    }
}

#[cfg(target_arch = "wasm32")]
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

#[cfg(target_arch = "wasm32")]
fn render_cranelift_function_from_bb(function: &bb_ir::BbFunction) -> Result<String, String> {
    if function.blocks.is_empty() {
        return Err("function has no blocks".to_string());
    }
    let entry_block = function
        .blocks
        .iter()
        .find(|block| block.label == function.entry)
        .ok_or_else(|| format!("missing entry block: {}", function.entry))?;

    let mut func = ir::Function::new();
    func.name = UserFuncName::testcase(sanitize_clif_testcase_name(
        function.qualname.as_str(),
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
        label_to_params.insert(block.label.clone(), block.params.clone());
    }

    for block in &function.blocks {
        let clif_block = *label_to_block
            .get(block.label.as_str())
            .expect("block label must exist");
        if block.label == function.entry {
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
            .params
            .iter()
            .zip(builder.block_params(clif_block).iter().copied())
        {
            current_values.insert(name.clone(), value);
        }

        // Preserve a stable one-op-per-source-op shape for web visualization.
        for _ in &block.ops {
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
            bb_ir::BbTerm::TryJump { body_label, .. } => {
                let body_block = *label_to_block
                    .get(body_label.as_str())
                    .ok_or_else(|| format!("missing try_jump body target: {body_label}"))?;
                let args = clif_target_args_for_block(
                    &mut builder,
                    body_label.as_str(),
                    &current_values,
                    &label_to_params,
                );
                builder.ins().jump(body_block, &args);
            }
        }
    }

    builder.seal_all_blocks();
    builder.finalize();
    Ok(func.display().to_string())
}

#[cfg(target_arch = "wasm32")]
fn bb_module_to_clif(module: &bb_ir::BbModule) -> String {
    if module.functions.is_empty() {
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
    out.push_str("decl @dp_jit_run_bb_step(pyobj, pyobj) -> pyobj\n");
    out.push_str("decl @dp_jit_term_kind(pyobj) -> termkind\n");
    out.push_str("decl @dp_jit_term_jump_target(pyobj) -> pyobj\n");
    out.push_str("decl @dp_jit_term_jump_args(pyobj) -> pyobj\n");
    out.push_str("decl @dp_jit_term_ret_value(pyobj) -> pyobj\n");
    out.push_str("decl @dp_jit_term_raise(pyobj) -> i32\n");
    out.push_str("decl @dp_jit_term_invalid(pyobj) -> i32\n");
    out.push('\n');
    out.push_str("; generated function declarations\n");
    for function in &module.functions {
        let params = function
            .param_names
            .iter()
            .map(|name| format!("%{name}: pyobj"))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "decl %{}({params}) -> pyobj ; bind={} target={:?}\n",
            function.qualname, function.bind_name, function.binding_target
        ));
    }
    if let Some(module_init) = module.module_init.as_ref() {
        out.push_str(&format!("decl %{module_init}() -> pyobj ; module_init\n"));
    }
    out.push('\n');

    out.push_str("; ---- rendered with Cranelift Function::display() ----\n");
    for function in &module.functions {
        let mut label_to_index = HashMap::new();
        let mut label_to_params = HashMap::new();
        for (index, block) in function.blocks.iter().enumerate() {
            label_to_index.insert(block.label.clone(), index);
            label_to_params.insert(block.label.clone(), block.params.clone());
        }

        out.push_str(&format!(
            "; function {} (kind={:?}, bind={}, target={:?}, entry={})\n",
            function.qualname,
            function.kind,
            function.bind_name,
            function.binding_target,
            function.entry
        ));
        for block in &function.blocks {
            out.push_str(&format!(
                ";   {}({})\n",
                block.label,
                block
                    .params
                    .iter()
                    .map(|name| format!("%{name}: pyobj"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            if let Some(exc_target) = block.exc_target_label.as_ref() {
                out.push_str(&format!(
                    ";     exc_target={}\n",
                    clif_target_comment(exc_target, &label_to_index, &label_to_params)
                ));
            }
            if let Some(exc_name) = block.exc_name.as_ref() {
                out.push_str(&format!(";     exc_name=%{exc_name}\n"));
            }
            let ops = ruff_ast_to_string(&bb_ir::bb_ops_to_stmts(&block.ops));
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
