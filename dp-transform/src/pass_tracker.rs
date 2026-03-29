use crate::block_py::pretty::BlockPyPrettyPrint;
use crate::block_py::BlockPyModule;
use crate::passes::ast_to_ast::body::Suite;
use crate::passes::{CoreBlockPyPass, ResolvedStorageBlockPyPass, RuffBlockPyPass};
use ruff_python_ast::{self as ast, ModModule};
use ruff_text_size::TextRange;
use std::any::Any;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct PassTiming {
    pub name: String,
    pub elapsed: Duration,
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
        crate::ruff_ast_to_string(self)
    }
}

impl BlockPyPrettyPrint for ModModule {
    fn pretty_print(&self) -> String {
        crate::ruff_ast_to_string(&self.body)
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
        let start = Instant::now();
        let value = build();
        let elapsed = start.elapsed();
        self.record_pass_timing(name, elapsed);
        value
    }
}
