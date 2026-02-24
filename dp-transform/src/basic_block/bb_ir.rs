use ruff_python_ast::{
    Expr, Stmt, StmtAssign, StmtDelete, StmtExpr, StmtFunctionDef, StmtIf,
};

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
        resume_label: String,
        target_labels: Vec<String>,
        resume_pcs: Vec<(String, usize)>,
    },
    AsyncGenerator {
        resume_label: String,
        target_labels: Vec<String>,
        resume_pcs: Vec<(String, usize)>,
    },
}

#[derive(Debug, Clone)]
pub struct BbBlock {
    pub label: String,
    pub params: Vec<String>,
    pub ops: Vec<BbOp>,
    pub exc_target_label: Option<String>,
    pub exc_name: Option<String>,
    pub term: BbTerm,
}

#[derive(Debug, Clone)]
pub enum BbOp {
    Assign(StmtAssign),
    Expr(StmtExpr),
    Delete(StmtDelete),
    FunctionDef(StmtFunctionDef),
    If(StmtIf),
}

impl BbOp {
    pub fn from_stmt(stmt: Stmt) -> Option<Self> {
        match stmt {
            Stmt::Assign(assign) => Some(Self::Assign(assign)),
            Stmt::Expr(expr) => Some(Self::Expr(expr)),
            Stmt::Delete(delete) => Some(Self::Delete(delete)),
            Stmt::Pass(_) => None,
            Stmt::FunctionDef(function_def) => Some(Self::FunctionDef(function_def)),
            Stmt::If(if_stmt) => Some(Self::If(if_stmt)),
            other => panic!("unsupported statement in BbBlock.ops: {other:?}"),
        }
    }

    pub fn to_stmt(&self) -> Stmt {
        match self {
            Self::Assign(assign) => Stmt::Assign(assign.clone()),
            Self::Expr(expr) => Stmt::Expr(expr.clone()),
            Self::Delete(delete) => Stmt::Delete(delete.clone()),
            Self::FunctionDef(function_def) => Stmt::FunctionDef(function_def.clone()),
            Self::If(if_stmt) => Stmt::If(if_stmt.clone()),
        }
    }
}

pub fn bb_ops_to_stmts(ops: &[BbOp]) -> Vec<Stmt> {
    ops.iter().map(BbOp::to_stmt).collect()
}

#[derive(Debug, Clone)]
pub enum BbTerm {
    Jump(String),
    BrIf {
        test: Expr,
        then_label: String,
        else_label: String,
    },
    BrTable {
        index: Expr,
        targets: Vec<String>,
        default_label: String,
    },
    Raise {
        exc: Option<Expr>,
        cause: Option<Expr>,
    },
    TryJump {
        body_label: String,
        except_label: String,
        except_exc_name: Option<String>,
        body_region_labels: Vec<String>,
        except_region_labels: Vec<String>,
        finally_label: Option<String>,
        finally_exc_name: Option<String>,
        finally_region_labels: Vec<String>,
        finally_fallthrough_label: Option<String>,
    },
    Ret(Option<Expr>),
}
