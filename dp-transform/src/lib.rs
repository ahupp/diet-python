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

pub mod bb_ir;
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

    let payload = json!({
        "phase1": phase_one.to_string(),
        "bbRaw": bb.to_string(),
        "bbModule": bb_module_json,
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
                    let ops_text = ruff_ast_to_string(&block.ops).trim().to_string();
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
            start_pc,
            target_labels,
            throw_dispatch_pcs,
        } => json!({
            "kind": "generator",
            "startPc": start_pc,
            "targetLabels": target_labels,
            "throwDispatchPcs": throw_dispatch_pcs,
        }),
        bb_ir::BbFunctionKind::AsyncGenerator {
            start_pc,
            target_labels,
            throw_dispatch_pcs,
        } => json!({
            "kind": "async_generator",
            "startPc": start_pc,
            "targetLabels": target_labels,
            "throwDispatchPcs": throw_dispatch_pcs,
        }),
    }
}

#[cfg(target_arch = "wasm32")]
fn bb_term_kind(term: &bb_ir::BbTerm) -> &'static str {
    match term {
        bb_ir::BbTerm::Jump(_) => "jump",
        bb_ir::BbTerm::BrIf { .. } => "br_if",
        bb_ir::BbTerm::Raise { .. } => "raise",
        bb_ir::BbTerm::TryJump { .. } => "try_jump",
        bb_ir::BbTerm::Yield { .. } => "yield",
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
        bb_ir::BbTerm::Yield {
            value,
            resume_label,
        } => {
            let value = value
                .as_ref()
                .map(expr_to_one_line)
                .unwrap_or_else(|| "None".to_string());
            format!("yield {value} -> {resume_label}")
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
        bb_ir::BbTerm::Yield { resume_label, .. } => vec![(resume_label.as_str(), "yield_resume")],
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
