use crate::block_py::pretty::BlockPyPrettyPrint;
use crate::passes::ast_to_ast::body::Suite;
use crate::passes::{CoreBlockPyPass, ResolvedStorageBlockPyPass, RuffBlockPyPass};
use anyhow::Result;
use ruff_python_ast::{self as ast, Expr, ModModule, Stmt};
use ruff_python_codegen::{Generator, Indentation};
use ruff_python_parser::parse_module;
pub use ruff_python_parser::ParseError;
use ruff_source_file::LineEnding;
use ruff_text_size::TextRange;
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
#[cfg(target_arch = "wasm32")]
mod web_inspector;

use crate::block_py::BlockPyModule;
use crate::driver::rewrite_module_with_tracker;

#[derive(Debug, Clone)]
pub struct PassTiming {
    pub name: String,
    pub elapsed: Duration,
}

#[derive(Debug, Clone)]
pub struct TransformTimings {
    pub total_time: Duration,
    pub pass_times: Vec<PassTiming>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PassShapeSummary {
    pub contains_await: bool,
    pub contains_yield: bool,
    pub contains_dp_add: bool,
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
    pub timings: TransformTimings,
    pub module: ModModule,
    pub bb_codegen_module: Option<BlockPyModule<ResolvedStorageBlockPyPass>>,
    pub pass_tracker: P,
}

struct TrackedPass {
    name: String,
    elapsed: Duration,
    value: Box<dyn Any>,
    render_text: Option<fn(&dyn Any) -> String>,
}

#[derive(Default)]
pub struct NoopPassTracker;

pub struct RecordingPassTracker {
    passes: Vec<TrackedPass>,
}

pub(crate) trait TrackedPassText {
    fn render_tracked_pass_text(&self) -> String;
}

pub(crate) trait PassTracker {
    fn run_pass<T, F>(&mut self, name: &str, build: F) -> T
    where
        T: Clone + Any + TrackedPassText,
        F: FnOnce() -> T;
}

impl TrackedPassText for Suite {
    fn render_tracked_pass_text(&self) -> String {
        ruff_ast_to_string(self)
    }
}

impl TrackedPassText for ModModule {
    fn render_tracked_pass_text(&self) -> String {
        ruff_ast_to_string(&self.body)
    }
}

impl<T> TrackedPassText for T
where
    T: BlockPyPrettyPrint,
{
    fn render_tracked_pass_text(&self) -> String {
        self.pretty_print()
    }
}

fn render_tracked_pass_value<T>(value: &dyn Any) -> String
where
    T: Any + TrackedPassText,
{
    value
        .downcast_ref::<T>()
        .expect("tracked pass renderer type should match stored value")
        .render_tracked_pass_text()
}

impl NoopPassTracker {
    pub fn new() -> Self {
        Self
    }
}

impl PassTracker for NoopPassTracker {
    fn run_pass<T, F>(&mut self, _name: &str, build: F) -> T
    where
        T: Clone + Any + TrackedPassText,
        F: FnOnce() -> T,
    {
        build()
    }
}

impl RecordingPassTracker {
    pub fn new() -> Self {
        Self { passes: Vec::new() }
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

    pub fn summarize_pass_shape(&self, name: &str) -> Option<PassShapeSummary> {
        crate::passes::summarize_tracked_pass_shape(self, name)
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
        self.passes.iter().map(|pass| PassTiming {
            name: pass.name.clone(),
            elapsed: pass.elapsed,
        })
    }
}

impl PassTracker for RecordingPassTracker {
    fn run_pass<T, F>(&mut self, name: &str, build: F) -> T
    where
        T: Clone + Any + TrackedPassText,
        F: FnOnce() -> T,
    {
        let start = timing_start();
        let value = build();
        let elapsed = timing_elapsed(start);
        assert!(
            !self.passes.iter().any(|pass| pass.name == name),
            "PassTracker already contains a pass named {name}",
        );
        self.passes.push(TrackedPass {
            name: name.to_string(),
            elapsed,
            value: Box::new(value.clone()),
            render_text: Some(render_tracked_pass_value::<T>),
        });
        value
    }
}

impl<P> LoweringResult<P> {
    pub fn to_string(&self) -> String {
        ruff_ast_to_string(&self.module.body)
    }

    pub fn invalid_future_feature(&self) -> Option<String> {
        let body = &self.module.body;
        let [Stmt::Global(global_stmt), Stmt::Nonlocal(nonlocal_stmt), ..] = &body[..] else {
            return None;
        };
        let [global_name] = global_stmt.names.as_slice() else {
            return None;
        };
        let [nonlocal_name] = nonlocal_stmt.names.as_slice() else {
            return None;
        };
        (global_name == nonlocal_name).then(|| global_name.id.to_string())
    }
}

struct LoweringCore {
    module: ModModule,
    bb_codegen_module: Option<BlockPyModule<ResolvedStorageBlockPyPass>>,
    total_time: Duration,
}

fn lower_source_with_tracker(
    source: &str,
    pass_tracker: &mut impl PassTracker,
) -> Result<LoweringCore> {
    init_logging();
    namegen::reset_namegen_state();

    if should_skip(source) {
        return Ok(LoweringCore {
            module: parse_module(source)?.into_syntax(),
            bb_codegen_module: None,
            total_time: Duration::ZERO,
        });
    }

    let total_start = timing_start();
    let (module, bb_codegen_module) = rewrite_module_with_tracker(source, pass_tracker)?;

    Ok(LoweringCore {
        module,
        bb_codegen_module: Some(bb_codegen_module),
        total_time: timing_elapsed(total_start),
    })
}

/// Transform the source code and return the resulting Ruff AST.
pub fn transform_str_to_ruff(source: &str) -> Result<LoweringResult> {
    let mut pass_tracker = RecordingPassTracker::new();
    let LoweringCore {
        module,
        bb_codegen_module,
        total_time,
    } = lower_source_with_tracker(source, &mut pass_tracker)?;
    let timings = TransformTimings {
        total_time,
        pass_times: pass_tracker.pass_timings().collect(),
    };
    Ok(LoweringResult {
        timings,
        module,
        bb_codegen_module,
        pass_tracker,
    })
}

pub fn transform_str_to_ruff_no_passes(source: &str) -> Result<LoweringResult<NoopPassTracker>> {
    let mut pass_tracker = NoopPassTracker::new();
    let LoweringCore {
        module,
        bb_codegen_module,
        total_time,
    } = lower_source_with_tracker(source, &mut pass_tracker)?;
    Ok(LoweringResult {
        timings: TransformTimings {
            total_time,
            pass_times: Vec::new(),
        },
        module,
        bb_codegen_module,
        pass_tracker,
    })
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
