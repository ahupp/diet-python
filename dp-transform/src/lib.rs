use ruff_python_ast::{self as ast, Expr, Stmt, StmtBody};
use ruff_python_codegen::{Generator, Indentation};
use ruff_python_parser::parse_module;
pub use ruff_python_parser::ParseError;
use ruff_source_file::LineEnding;
use ruff_text_size::TextRange;
use std::fmt::Display;
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
use crate::basic_block::block_py::BlockPyModule;
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
    pub blockpy_module: Option<BlockPyModule>,
    pub bb_module: Option<bb_ir::BbModule>,
}

pub(crate) struct PassTracker {
    passes: Vec<(String, String)>,
}

impl PassTracker {
    pub(crate) fn enabled() -> Self {
        Self { passes: Vec::new() }
    }

    pub(crate) fn add_pass(&mut self, name: &str, res: &impl Display) {
        self.passes.push((name.to_string(), res.to_string()));
    }

    pub(crate) fn rendered(&self, name: &str) -> Option<&str> {
        self.passes
            .iter()
            .rev()
            .find(|(pass_name, _)| pass_name == name)
            .map(|(_, rendered)| rendered.as_str())
    }
}

impl LoweringResult {
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
    transform_str_to_ruff_with_options_and_tracker(source, options, None)
}

pub(crate) fn transform_str_to_ruff_with_options_and_tracker(
    source: &str,
    options: Options,
    pass_tracker: Option<&mut PassTracker>,
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
            blockpy_module: None,
            bb_module: None,
        });
    }

    let ctx = Context::new(options, source);

    let rewrite_start = timing_start();
    let (blockpy_module, bb_module) =
        rewrite_module_with_tracker(&ctx, &mut module.body, pass_tracker);
    let rewrite_time = timing_elapsed(rewrite_start);

    let timings = TransformTimings {
        parse_time,
        rewrite_time,
        total_time: timing_elapsed(total_start),
    };

    Ok(LoweringResult {
        timings,
        module,
        blockpy_module: Some(blockpy_module),
        bb_module: Some(bb_module),
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
