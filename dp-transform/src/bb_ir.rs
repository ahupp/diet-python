use ruff_python_ast::{Expr, Stmt};

#[derive(Debug, Clone)]
pub struct BbModule {
    pub functions: Vec<BbFunction>,
    pub module_init: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BbFunction {
    pub bind_name: String,
    pub display_name: String,
    pub qualname: String,
    pub binding_target: BbBindingTarget,
    pub kind: BbFunctionKind,
    pub entry: String,
    pub param_names: Vec<String>,
    pub entry_params: Vec<String>,
    pub param_specs: Expr,
    pub local_cell_slots: Vec<String>,
    pub blocks: Vec<BbBlock>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BbBindingTarget {
    Local,
    ModuleGlobal,
    ClassNamespace,
}

#[derive(Debug, Clone)]
pub enum BbFunctionKind {
    Function,
    Coroutine,
    Generator {
        start_pc: usize,
        target_labels: Vec<String>,
        throw_dispatch_pcs: Vec<Option<usize>>,
    },
    AsyncGenerator {
        start_pc: usize,
        target_labels: Vec<String>,
        throw_dispatch_pcs: Vec<Option<usize>>,
    },
}

#[derive(Debug, Clone)]
pub struct BbBlock {
    pub label: String,
    pub params: Vec<String>,
    pub ops: Vec<Stmt>,
    pub term: BbTerm,
}

#[derive(Debug, Clone)]
pub enum BbTerm {
    Jump(String),
    BrIf {
        test: Expr,
        then_label: String,
        else_label: String,
    },
    Raise {
        exc: Option<Expr>,
        cause: Option<Expr>,
    },
    TryJump {
        body_label: String,
        except_label: String,
        body_region_labels: Vec<String>,
        except_region_labels: Vec<String>,
        finally_label: Option<String>,
        finally_region_labels: Vec<String>,
        finally_fallthrough_label: Option<String>,
    },
    Yield {
        value: Option<Expr>,
        resume_label: String,
    },
    Ret(Option<Expr>),
}
