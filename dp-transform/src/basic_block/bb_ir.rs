use super::block_py::CoreBlockPyExprWithoutAwaitOrYield;
use super::cfg_ir::{CfgBlock, CfgCallableDef, CfgModule};
use super::lowered_ir::{BindingTarget, ClosureLayout, FunctionId, LoweredFunctionKind};
use crate::py_expr;
use ruff_python_ast::{
    self as ast, AtomicNodeIndex, Expr, ExprContext, ExprName, Stmt, StmtAssign, StmtDelete,
    StmtExpr, StmtFunctionDef,
};
use ruff_text_size::TextRange;
use std::ops::{Deref, DerefMut};

pub type BbModule = CfgModule<BbFunction>;

#[derive(Debug, Clone)]
pub struct BbFunction {
    pub cfg: CfgCallableDef<FunctionId, LoweredFunctionKind, Vec<String>, BbBlock>,
    pub binding_target: BindingTarget,
    pub is_coroutine: bool,
    pub closure_layout: Option<ClosureLayout>,
    pub local_cell_slots: Vec<String>,
}

impl Deref for BbFunction {
    type Target = CfgCallableDef<FunctionId, LoweredFunctionKind, Vec<String>, BbBlock>;

    fn deref(&self) -> &Self::Target {
        &self.cfg
    }
}

impl DerefMut for BbFunction {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cfg
    }
}

#[derive(Debug, Clone, Default)]
pub struct BbBlockMeta {
    pub params: Vec<String>,
    pub local_defs: Vec<StmtFunctionDef>,
    pub exc_target_label: Option<String>,
    pub exc_name: Option<String>,
}

pub type BbBlock = CfgBlock<String, BbOp, BbTerm, BbBlockMeta>;

#[derive(Debug, Clone)]
pub enum BbOp {
    Assign(BbAssignOp),
    Expr(BbExprOp),
    Delete(BbDeleteOp),
}

#[derive(Debug, Clone)]
pub struct BbAssignOp {
    pub node_index: AtomicNodeIndex,
    pub range: TextRange,
    pub target: ExprName,
    pub value: CoreBlockPyExprWithoutAwaitOrYield,
}

#[derive(Debug, Clone)]
pub struct BbExprOp {
    pub node_index: AtomicNodeIndex,
    pub range: TextRange,
    pub value: CoreBlockPyExprWithoutAwaitOrYield,
}

#[derive(Debug, Clone)]
pub struct BbDeleteOp {
    pub node_index: AtomicNodeIndex,
    pub range: TextRange,
    pub targets: Vec<CoreBlockPyExprWithoutAwaitOrYield>,
}

impl BbOp {
    pub fn from_stmt(stmt: Stmt) -> Option<Self> {
        match stmt {
            Stmt::Assign(assign) => {
                let [target] = assign.targets.as_slice() else {
                    panic!("unsupported assignment form in BbBlock.body: {assign:?}");
                };
                let value = *assign.value;
                match target {
                    Expr::Name(target) => Some(Self::Assign(BbAssignOp {
                        node_index: assign.node_index,
                        range: assign.range,
                        target: target.clone(),
                        value: CoreBlockPyExprWithoutAwaitOrYield::from_expr(value),
                    })),
                    Expr::Attribute(target) => Some(Self::Expr(BbExprOp {
                        node_index: assign.node_index,
                        range: assign.range,
                        value: CoreBlockPyExprWithoutAwaitOrYield::from_expr(py_expr!(
                            "__dp_setattr({obj:expr}, {attr:literal}, {value:expr})",
                            obj = *target.value.clone(),
                            attr = target.attr.as_str(),
                            value = value,
                        )),
                    })),
                    Expr::Subscript(target) => Some(Self::Expr(BbExprOp {
                        node_index: assign.node_index,
                        range: assign.range,
                        // Assignment targets like `l[0] = ...` still evaluate `l`
                        // as a load first; preserve UnboundLocalError semantics
                        // when `l` is a deleted/unbound local sentinel.
                        value: CoreBlockPyExprWithoutAwaitOrYield::from_expr(py_expr!(
                            "__dp_setitem({obj:expr}, {idx:expr}, {value:expr})",
                            obj = if let Expr::Name(name) = target.value.as_ref() {
                                py_expr!(
                                    "__dp_load_deleted_name({name:literal}, {value:expr})",
                                    name = name.id.as_str(),
                                    value = *target.value.clone(),
                                )
                            } else {
                                *target.value.clone()
                            },
                            idx = *target.slice.clone(),
                            value = value,
                        )),
                    })),
                    _ => panic!("unsupported assignment target in BbBlock.body"),
                }
            }
            Stmt::Expr(expr) => Some(Self::Expr(BbExprOp {
                node_index: expr.node_index,
                range: expr.range,
                value: CoreBlockPyExprWithoutAwaitOrYield::from_expr(*expr.value),
            })),
            Stmt::Delete(delete) => Some(Self::Delete(BbDeleteOp {
                node_index: delete.node_index,
                range: delete.range,
                targets: delete
                    .targets
                    .into_iter()
                    .map(CoreBlockPyExprWithoutAwaitOrYield::from_expr)
                    .collect(),
            })),
            Stmt::Pass(_) => None,
            Stmt::FunctionDef(_) => panic!(
                "FunctionDef is not allowed in BbBlock.body; lower to binding statements first"
            ),
            other => panic!("unsupported statement in BbBlock.body: {other:?}"),
        }
    }

    pub fn to_stmt(&self) -> Stmt {
        match self {
            Self::Assign(assign) => Stmt::Assign(StmtAssign {
                node_index: assign.node_index.clone(),
                range: assign.range,
                targets: vec![Expr::Name(assign.target.clone())],
                value: Box::new(assign.value.to_expr()),
            }),
            Self::Expr(expr) => Stmt::Expr(StmtExpr {
                node_index: expr.node_index.clone(),
                range: expr.range,
                value: Box::new(expr.value.to_expr()),
            }),
            Self::Delete(delete) => Stmt::Delete(StmtDelete {
                node_index: delete.node_index.clone(),
                range: delete.range,
                targets: delete.targets.iter().map(|expr| expr.to_expr()).collect(),
            }),
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
        test: CoreBlockPyExprWithoutAwaitOrYield,
        then_label: String,
        else_label: String,
    },
    BrTable {
        index: CoreBlockPyExprWithoutAwaitOrYield,
        targets: Vec<String>,
        default_label: String,
    },
    Raise {
        exc: Option<CoreBlockPyExprWithoutAwaitOrYield>,
        cause: Option<CoreBlockPyExprWithoutAwaitOrYield>,
    },
    Ret(Option<CoreBlockPyExprWithoutAwaitOrYield>),
}
