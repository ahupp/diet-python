use crate::block_py::pretty::BlockPyPrettyPrint;
use crate::passes::ast_to_ast::body::Suite;
use crate::passes::{
    CodegenBlockPyPass, CoreBlockPyPass, ResolvedStorageBlockPyPass, RuffBlockPyPass,
};
use anyhow::Error as AnyhowError;
use ruff_python_ast::{self as ast, Expr, ModModule, Stmt};
use ruff_python_codegen::{Generator, Indentation};
pub use ruff_python_parser::ParseError;
use ruff_source_file::LineEnding;
use ruff_text_size::TextRange;
use serde_json::{json, Value};
use std::any::Any;
use std::sync::Once;
use std::time::{Duration, Instant};

pub mod block_py;
mod driver;
pub mod fixture;
mod namegen;
pub mod passes;
mod template;
#[cfg(test)]
mod test_util;
pub(crate) mod transformer;

use crate::block_py::BlockPyModule;
use crate::driver::rewrite_module_with_tracker;

#[derive(Debug)]
pub enum LoweringError {
    Parse(ParseError),
    Other(AnyhowError),
}

pub type Result<T> = std::result::Result<T, LoweringError>;

impl std::fmt::Display for LoweringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Parse(err) => err.fmt(f),
            Self::Other(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for LoweringError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Parse(err) => Some(err),
            Self::Other(err) => Some(err.as_ref()),
        }
    }
}

impl From<ParseError> for LoweringError {
    fn from(value: ParseError) -> Self {
        Self::Parse(value)
    }
}

impl From<AnyhowError> for LoweringError {
    fn from(value: AnyhowError) -> Self {
        Self::Other(value)
    }
}

#[derive(Debug, Clone)]
pub struct PassTiming {
    pub name: String,
    pub elapsed: Duration,
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

pub(crate) fn should_skip(source: &str) -> bool {
    source
        .lines()
        .next()
        .is_some_and(|line| line.contains("diet-python: disabled"))
}

pub struct LoweringResult<P = RecordingPassTracker> {
    pub total_time: Duration,
    pub codegen_module: Option<BlockPyModule<CodegenBlockPyPass>>,
    pub pass_tracker: P,
}

struct TrackedPass {
    name: String,
    value: Box<dyn Any>,
    render_text: Option<fn(&dyn Any) -> String>,
}

#[derive(Default)]
pub struct NoopPassTracker;

pub struct RecordingPassTracker {
    passes: Vec<TrackedPass>,
    timings: Vec<PassTiming>,
}

pub(crate) trait PassTracker {
    fn run_pass<T, F>(&mut self, name: &str, build: F) -> T
    where
        T: Clone + Any + BlockPyPrettyPrint,
        F: FnOnce() -> T;

    fn record_timing<T, F>(&mut self, name: &str, build: F) -> T
    where
        F: FnOnce() -> T;
}

impl BlockPyPrettyPrint for Suite {
    fn pretty_print(&self) -> String {
        ruff_ast_to_string(self)
    }
}

impl BlockPyPrettyPrint for ModModule {
    fn pretty_print(&self) -> String {
        ruff_ast_to_string(&self.body)
    }
}

fn render_tracked_pass_value<T>(value: &dyn Any) -> String
where
    T: Any + BlockPyPrettyPrint,
{
    value
        .downcast_ref::<T>()
        .expect("tracked pass renderer type should match stored value")
        .pretty_print()
}

impl NoopPassTracker {
    pub fn new() -> Self {
        Self
    }
}

impl PassTracker for NoopPassTracker {
    fn run_pass<T, F>(&mut self, _name: &str, build: F) -> T
    where
        T: Clone + Any + BlockPyPrettyPrint,
        F: FnOnce() -> T,
    {
        build()
    }

    fn record_timing<T, F>(&mut self, _name: &str, build: F) -> T
    where
        F: FnOnce() -> T,
    {
        build()
    }
}

impl RecordingPassTracker {
    pub fn new() -> Self {
        Self {
            passes: Vec::new(),
            timings: Vec::new(),
        }
    }

    fn record_pass_timing(&mut self, name: &str, elapsed: Duration) {
        assert!(
            !self.timings.iter().any(|timing| timing.name == name),
            "PassTracker already contains a pass named {name}",
        );
        self.timings.push(PassTiming {
            name: name.to_string(),
            elapsed,
        });
    }

    pub fn get<T: Any>(&self, name: &str) -> Option<&T> {
        self.passes
            .iter()
            .find(|pass| pass.name == name)
            .and_then(|pass| pass.value.downcast_ref::<T>())
    }

    pub fn pass_names(&self) -> impl Iterator<Item = &str> {
        self.passes.iter().map(|pass| pass.name.as_str())
    }

    pub fn pass_ast_to_ast(&self) -> Option<ModModule> {
        self.get::<crate::driver::AstToAstPassResult>("ast-to-ast")
            .map(|pass| ModModule {
                node_index: ast::AtomicNodeIndex::default(),
                range: TextRange::default(),
                body: pass.module.clone(),
            })
    }

    pub fn pass_semantic_blockpy(&self) -> Option<&BlockPyModule<RuffBlockPyPass>> {
        self.get::<BlockPyModule<RuffBlockPyPass>>("semantic_blockpy")
    }

    pub fn pass_core_blockpy(&self) -> Option<&BlockPyModule<CoreBlockPyPass>> {
        self.get::<BlockPyModule<CoreBlockPyPass>>("core_blockpy")
    }

    pub fn pass_name_binding(&self) -> Option<&BlockPyModule<ResolvedStorageBlockPyPass>> {
        self.get::<BlockPyModule<ResolvedStorageBlockPyPass>>("name_binding")
    }

    pub fn render_pass_text(&self, name: &str) -> Option<String> {
        let pass = self.passes.iter().find(|pass| pass.name == name)?;
        pass.render_text.map(|render| render(pass.value.as_ref()))
    }

    pub fn pass_timings(&self) -> impl Iterator<Item = PassTiming> + '_ {
        self.timings.iter().cloned()
    }
}

impl PassTracker for RecordingPassTracker {
    fn run_pass<T, F>(&mut self, name: &str, build: F) -> T
    where
        T: Clone + Any + BlockPyPrettyPrint,
        F: FnOnce() -> T,
    {
        let value = self.record_timing(name, build);
        self.passes.push(TrackedPass {
            name: name.to_string(),
            value: Box::new(value.clone()),
            render_text: Some(render_tracked_pass_value::<T>),
        });
        value
    }

    fn record_timing<T, F>(&mut self, name: &str, build: F) -> T
    where
        F: FnOnce() -> T,
    {
        let start = timing_start();
        let value = build();
        let elapsed = timing_elapsed(start);
        self.record_pass_timing(name, elapsed);
        value
    }
}

fn inspector_function_payload(
    function: &crate::block_py::BlockPyFunction<CodegenBlockPyPass>,
) -> Value {
    json!({
        "functionId": function.function_id.0,
        "qualname": function.names.qualname,
        "displayName": function.names.display_name,
        "bindName": function.names.bind_name,
        "kind": format!("{:?}", function.kind).to_lowercase(),
        "entryLabel": function.entry_block().label_str(),
    })
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
    let functions = output
        .codegen_module
        .as_ref()
        .map(|module| {
            module
                .callable_defs
                .iter()
                .map(inspector_function_payload)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({
        "steps": steps,
        "functions": functions,
    })
    .to_string()
}

fn lower_python_to_blockpy_with_tracker<P>(
    source: &str,
    mut pass_tracker: P,
) -> Result<LoweringResult<P>>
where
    P: PassTracker,
{
    init_logging();
    namegen::reset_namegen_state();
    let total_start = timing_start();

    if should_skip(source) {
        return Ok(LoweringResult {
            total_time: timing_elapsed(total_start),
            codegen_module: None,
            pass_tracker,
        });
    }

    let codegen_module = rewrite_module_with_tracker(source, &mut pass_tracker)?;

    Ok(LoweringResult {
        total_time: timing_elapsed(total_start),
        codegen_module: Some(codegen_module),
        pass_tracker,
    })
}

pub fn lower_python_to_blockpy_recorded(source: &str) -> Result<LoweringResult> {
    lower_python_to_blockpy_with_tracker(source, RecordingPassTracker::new())
}

pub fn lower_python_to_blockpy(source: &str) -> Result<LoweringResult<NoopPassTracker>> {
    lower_python_to_blockpy_with_tracker(source, NoopPassTracker::new())
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

#[cfg(test)]
mod test;

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
