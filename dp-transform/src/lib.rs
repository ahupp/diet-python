use ruff_python_ast::{self as ast, Expr, Stmt, StmtBody};
use ruff_python_codegen::{Generator, Indentation};
use ruff_python_parser::parse_module;
pub use ruff_python_parser::ParseError;
use ruff_source_file::LineEnding;
use ruff_text_size::TextRange;
use std::any::{type_name, Any};
use std::sync::Once;
use std::time::{Duration, Instant};

pub mod basic_block;
mod driver;
pub mod fixture;
mod namegen;
mod template;
#[cfg(test)]
mod test_util;
pub(crate) mod transformer;
#[cfg(target_arch = "wasm32")]
mod web_inspector;

use crate::basic_block::ast_to_ast::context::Context;
pub use crate::basic_block::ast_to_ast::scope::{analyze_module_scope, Scope};
pub use crate::basic_block::ast_to_ast::Options;
use crate::basic_block::bb_ir;
use crate::driver::rewrite_module_with_tracker;

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
    passes: PassTracker,
}

#[derive(Clone)]
pub(crate) struct RewrittenAstAfterInitialSimplify(pub ruff_python_ast::StmtBody);

struct TrackedPass {
    name: String,
    value: Box<dyn Any>,
}

pub(crate) struct PassTracker {
    passes: Vec<TrackedPass>,
}

impl PassTracker {
    pub(crate) fn new() -> Self {
        Self { passes: Vec::new() }
    }

    pub(crate) fn add_pass<T: Clone + Any>(&mut self, name: &str, build: impl FnOnce() -> T) -> T {
        let value = build();
        assert!(
            !self.passes.iter().any(|pass| pass.value.is::<T>()),
            "PassTracker already contains a pass for type {}",
            type_name::<T>()
        );
        self.passes.push(TrackedPass {
            name: name.to_string(),
            value: Box::new(value.clone()),
        });
        value
    }

    pub(crate) fn get<T: Any>(&self) -> Option<&T> {
        self.passes
            .iter()
            .rev()
            .find_map(|pass| pass.value.downcast_ref::<T>())
    }

    fn names(&self) -> impl Iterator<Item = &str> {
        self.passes.iter().map(|pass| pass.name.as_str())
    }
}

impl LoweringResult {
    pub fn get_pass<T: Any>(&self) -> Option<&T> {
        self.passes.get::<T>()
    }

    pub fn pass_names(&self) -> impl Iterator<Item = &str> {
        self.passes.names()
    }

    pub fn to_string(&self) -> String {
        ruff_ast_to_string(&self.module.body)
    }

    pub fn module_docstring(&self) -> Option<String> {
        let stmt = self.module.body.body.first()?.as_ref();
        let Stmt::Expr(ast::StmtExpr { value, .. }) = stmt else {
            return None;
        };
        let Expr::StringLiteral(ast::ExprStringLiteral { value, .. }) = value.as_ref() else {
            return None;
        };
        Some(value.to_string())
    }
}

/// Transform the source code and return the resulting Ruff AST.
pub fn transform_str_to_ruff_with_options(
    source: &str,
    options: Options,
) -> Result<LoweringResult, ParseError> {
    init_logging();
    namegen::reset_namegen_state();
    let options = options;

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
            passes: PassTracker::new(),
        });
    }

    let ctx = Context::new(options, source);
    let mut pass_tracker = PassTracker::new();

    let rewrite_start = timing_start();
    let bb_module = rewrite_module_with_tracker(&ctx, &mut module.body, &mut pass_tracker);
    let rewrite_time = timing_elapsed(rewrite_start);

    let timings = TransformTimings {
        parse_time,
        rewrite_time,
        total_time: timing_elapsed(total_start),
    };

    Ok(LoweringResult {
        timings,
        module,
        bb_module: Some(bb_module),
        passes: pass_tracker,
    })
}

pub fn transform_str_to_bb_ir_with_options(
    source: &str,
    options: Options,
) -> Result<Option<bb_ir::BbModule>, ParseError> {
    let mut options = options;
    options.lower_attributes = true;
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

#[cfg(test)]
mod tests {
    use super::PassTracker;

    #[test]
    #[should_panic(expected = "PassTracker already contains a pass for type i32")]
    fn pass_tracker_rejects_duplicate_types() {
        let mut tracker = PassTracker::new();
        tracker.add_pass("one", || 1_i32);
        tracker.add_pass("two", || 2_i32);
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
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn transform(source: &str) -> Result<String, wasm_bindgen::JsValue> {
    web_inspector::transform(source)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn transform_selected(
    source: &str,
    transforms: js_sys::Array,
) -> Result<String, wasm_bindgen::JsValue> {
    web_inspector::transform_selected(source, transforms)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn inspect_pipeline(source: &str) -> Result<String, wasm_bindgen::JsValue> {
    web_inspector::inspect_pipeline(source)
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn available_transforms() -> js_sys::Array {
    web_inspector::available_transforms()
}
