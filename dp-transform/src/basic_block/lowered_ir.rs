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
pub struct LoweredCfgFunction<B> {
    pub cfg: CfgCallableDef<FunctionId, LoweredFunctionKind, Vec<String>, B>,
    pub binding_target: BindingTarget,
    pub closure_layout: Option<ClosureLayout>,
    pub local_cell_slots: Vec<String>,
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
