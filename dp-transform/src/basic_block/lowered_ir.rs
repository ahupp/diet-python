use super::cfg_ir::CfgCallableDef;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FunctionId(pub usize);

impl FunctionId {
    pub fn plan_qualname(self, qualname: &str) -> String {
        format!("{qualname}::__dp_fn_{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BindingTarget {
    Local,
    ModuleGlobal,
    ClassNamespace,
}

#[derive(Debug, Clone)]
pub enum LoweredFunctionKind {
    Function,
    Generator {
        closure_state: bool,
        resume_label: String,
        target_labels: Vec<String>,
        resume_pcs: Vec<(String, usize)>,
    },
    AsyncGenerator {
        closure_state: bool,
        resume_label: String,
        target_labels: Vec<String>,
        resume_pcs: Vec<(String, usize)>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureLayout {
    pub freevars: Vec<ClosureSlot>,
    pub cellvars: Vec<ClosureSlot>,
    pub runtime_cells: Vec<ClosureSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureSlot {
    pub logical_name: String,
    pub storage_name: String,
    pub init: ClosureInit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosureInit {
    InheritedCapture,
    Parameter,
    DeletedSentinel,
    RuntimePcUnstarted,
    RuntimeNone,
    Deferred,
}

#[derive(Debug, Clone)]
pub struct BoundCallable<C> {
    pub callable: C,
    pub binding_target: BindingTarget,
}

impl<C> BoundCallable<C> {
    pub fn binding_target(&self) -> BindingTarget {
        self.binding_target
    }

    pub fn with_binding_target(mut self, binding_target: BindingTarget) -> Self {
        self.binding_target = binding_target;
        self
    }

    pub fn map_callable<D>(&self, f: impl FnOnce(&C) -> D) -> BoundCallable<D> {
        BoundCallable {
            callable: f(&self.callable),
            binding_target: self.binding_target,
        }
    }
}

impl<C> Deref for BoundCallable<C> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.callable
    }
}

impl<C> DerefMut for BoundCallable<C> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.callable
    }
}

#[derive(Debug, Clone)]
pub struct LoweredCfgFunction<B> {
    pub cfg: BoundCallable<CfgCallableDef<FunctionId, LoweredFunctionKind, Vec<String>, B>>,
    pub closure_layout: Option<ClosureLayout>,
    pub local_cell_slots: Vec<String>,
}

impl<B> LoweredCfgFunction<B> {
    pub fn binding_target(&self) -> BindingTarget {
        self.cfg.binding_target()
    }
}

impl<B> Deref for LoweredCfgFunction<B> {
    type Target = CfgCallableDef<FunctionId, LoweredFunctionKind, Vec<String>, B>;

    fn deref(&self) -> &Self::Target {
        &self.cfg
    }
}

impl<B> DerefMut for LoweredCfgFunction<B> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cfg
    }
}
