use crate::py_expr;
use ruff_python_ast::{
    self as ast, AtomicNodeIndex, Expr, ExprContext, ExprName, Stmt, StmtAssign, StmtDelete,
    StmtExpr, StmtFunctionDef,
};
use ruff_python_parser::parse_expression;
use ruff_text_size::TextRange;

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
    pub binding_target: BindingTarget,
    pub is_coroutine: bool,
    pub kind: BbFunctionKind,
    pub entry: String,
    pub param_names: Vec<String>,
    pub entry_params: Vec<String>,
    pub closure_layout: Option<BbClosureLayout>,
    pub param_specs: BbExpr,
    pub local_cell_slots: Vec<String>,
    pub blocks: Vec<BbBlock>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BindingTarget {
    Local,
    ModuleGlobal,
    ClassNamespace,
}

#[derive(Debug, Clone)]
pub enum BbFunctionKind {
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
pub struct BbClosureLayout {
    pub inherited_captures: Vec<BbClosureSlot>,
    pub lifted_locals: Vec<BbClosureSlot>,
    pub runtime_cells: Vec<BbClosureSlot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BbClosureSlot {
    pub logical_name: String,
    pub storage_name: String,
    pub init: BbClosureInit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BbClosureInit {
    InheritedCapture,
    Parameter,
    DeletedSentinel,
    RuntimePcZero,
    RuntimeNone,
    Deferred,
}

#[derive(Debug, Clone)]
pub struct BbBlock {
    pub label: String,
    pub params: Vec<String>,
    pub local_defs: Vec<StmtFunctionDef>,
    pub ops: Vec<BbOp>,
    pub exc_target_label: Option<String>,
    pub exc_name: Option<String>,
    pub term: BbTerm,
}

#[derive(Debug, Clone)]
pub enum BbOp {
    Assign(BbAssignOp),
    Expr(BbExprOp),
    Delete(BbDeleteOp),
}

#[derive(Debug, Clone)]
pub enum BbExpr {
    Await(ast::ExprAwait),
    Call(BbCallExpr),
    FloatLiteral(ast::ExprNumberLiteral),
    IntLiteral(ast::ExprNumberLiteral),
    Name(ast::ExprName),
    BytesLiteral(ast::ExprBytesLiteral),
    Starred(ast::ExprStarred),
}

#[derive(Debug, Clone)]
pub struct BbCallExpr {
    pub template: ast::ExprCall,
    pub func: Box<BbExpr>,
    pub args: Vec<BbExpr>,
    pub keywords: Vec<BbExpr>,
}

impl BbCallExpr {
    fn to_expr_call(&self) -> ast::ExprCall {
        let mut call = self.template.clone();
        call.func = Box::new(self.func.to_expr());
        call.arguments.args = self.args.iter().map(BbExpr::to_expr).collect();
        if call.arguments.keywords.len() != self.keywords.len() {
            panic!(
                "BbCallExpr keyword metadata mismatch: template has {}, values have {}",
                call.arguments.keywords.len(),
                self.keywords.len()
            );
        }
        for (keyword, value) in call.arguments.keywords.iter_mut().zip(self.keywords.iter()) {
            keyword.value = value.to_expr();
        }
        call
    }
}

impl BbExpr {
    pub fn from_expr(expr: Expr) -> Self {
        let source = crate::ruff_ast_to_string(&expr);
        match expr {
            Expr::Await(value) => Self::Await(value),
            Expr::StringLiteral(value) => {
                return Self::from_expr(string_literal_to_decode_literal_bytes_expr(
                    value.value.to_string().as_str(),
                ));
            }
            Expr::Attribute(ast::ExprAttribute {
                value, attr, ctx, ..
            }) if matches!(ctx, ExprContext::Load) => {
                return Self::from_expr(py_expr!(
                    "__dp_getattr({obj:expr}, {attr:literal})",
                    obj = *value,
                    attr = attr.as_str(),
                ));
            }
            Expr::Subscript(ast::ExprSubscript {
                value, slice, ctx, ..
            }) if matches!(ctx, ExprContext::Load) => {
                return Self::from_expr(py_expr!(
                    "__dp_getitem({obj:expr}, {idx:expr})",
                    obj = *value,
                    idx = *slice,
                ));
            }
            Expr::Tuple(value) if matches!(value.ctx, ExprContext::Load) => {
                return Self::from_expr(make_dp_helper_call_expr("__dp_tuple", value.elts, vec![]));
            }
            Expr::List(_) | Expr::Set(_) | Expr::Dict(_) => panic!(
                "list/set/dict literals reached BbExpr::from_expr; these should be lowered before BB conversion: {}",
                source
            ),
            Expr::Starred(value) => Self::Starred(value),
            Expr::Call(value) => {
                let func = Box::new(Self::from_expr(*value.func.clone()));
                let args = value
                    .arguments
                    .args
                    .iter()
                    .cloned()
                    .map(Self::from_expr)
                    .collect();
                let keywords = value
                    .arguments
                    .keywords
                    .iter()
                    .map(|keyword| Self::from_expr(keyword.value.clone()))
                    .collect();
                Self::Call(BbCallExpr {
                    template: value,
                    func,
                    args,
                    keywords,
                })
            }
            Expr::Name(value) => Self::Name(value),
            Expr::BytesLiteral(value) => Self::BytesLiteral(value),
            Expr::NumberLiteral(value) => match value.value {
                ast::Number::Int(_) => Self::IntLiteral(value),
                ast::Number::Float(_) => Self::FloatLiteral(value),
                ast::Number::Complex { .. } => panic!(
                    "complex literal reached BbExpr::from_expr; this should be lowered earlier: {}",
                    source
                ),
            },
            Expr::BooleanLiteral(ast::ExprBooleanLiteral { value, .. }) => {
                if value {
                    return Self::from_expr(py_expr!("__dp_TRUE"));
                }
                return Self::from_expr(py_expr!("__dp_FALSE"));
            }
            Expr::NoneLiteral(_) => {
                return Self::from_expr(py_expr!("__dp_NONE"));
            }
            Expr::EllipsisLiteral(_) => {
                return Self::from_expr(py_expr!("__dp_Ellipsis"));
            }
            other => panic!(
                "unsupported expression in BbExpr::from_expr: {} ({other:?})",
                source
            ),
        }
    }

    pub fn to_expr(&self) -> Expr {
        match self {
            Self::Await(value) => Expr::Await(value.clone()),
            Self::Call(value) => Expr::Call(value.to_expr_call()),
            Self::FloatLiteral(value) => Expr::NumberLiteral(value.clone()),
            Self::IntLiteral(value) => Expr::NumberLiteral(value.clone()),
            Self::Name(value) => Expr::Name(value.clone()),
            Self::BytesLiteral(value) => Expr::BytesLiteral(value.clone()),
            Self::Starred(value) => Expr::Starred(value.clone()),
        }
    }
}

fn make_dp_helper_call_expr(
    helper_name: &str,
    args: Vec<Expr>,
    keywords: Vec<ast::Keyword>,
) -> Expr {
    let Expr::Call(mut call) = py_expr!("{helper:id}()", helper = helper_name) else {
        panic!("expected helper call expression for {helper_name}");
    };
    call.arguments.args = args.into();
    call.arguments.keywords = keywords.into();
    Expr::Call(call)
}

fn string_literal_to_decode_literal_bytes_expr(value: &str) -> Expr {
    let mut source = String::from("__dp_decode_literal_bytes(b\"");
    source.push_str(&escape_bytes_for_double_quoted_literal(value.as_bytes()));
    source.push_str("\")");
    let parsed = parse_expression(&source).unwrap_or_else(|err| {
        panic!("failed to build decoded-literal expression from {source:?}: {err}")
    });
    *parsed.into_syntax().body
}

fn escape_bytes_for_double_quoted_literal(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 4);
    for &byte in bytes {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\\""),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0x20..=0x7e => out.push(byte as char),
            _ => out.push_str(&format!("\\x{:02x}", byte)),
        }
    }
    out
}

#[derive(Debug, Clone)]
pub struct BbAssignOp {
    pub node_index: AtomicNodeIndex,
    pub range: TextRange,
    pub target: ExprName,
    pub value: BbExpr,
}

#[derive(Debug, Clone)]
pub struct BbExprOp {
    pub node_index: AtomicNodeIndex,
    pub range: TextRange,
    pub value: BbExpr,
}

#[derive(Debug, Clone)]
pub struct BbDeleteOp {
    pub node_index: AtomicNodeIndex,
    pub range: TextRange,
    pub targets: Vec<BbExpr>,
}

impl BbOp {
    pub fn from_stmt(stmt: Stmt) -> Option<Self> {
        match stmt {
            Stmt::Assign(assign) => {
                let [target] = assign.targets.as_slice() else {
                    panic!("unsupported assignment form in BbBlock.ops: {assign:?}");
                };
                let value = *assign.value;
                match target {
                    Expr::Name(target) => Some(Self::Assign(BbAssignOp {
                        node_index: assign.node_index,
                        range: assign.range,
                        target: target.clone(),
                        value: BbExpr::from_expr(value),
                    })),
                    Expr::Attribute(target) => Some(Self::Expr(BbExprOp {
                        node_index: assign.node_index,
                        range: assign.range,
                        value: BbExpr::from_expr(py_expr!(
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
                        value: BbExpr::from_expr(py_expr!(
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
                    _ => panic!("unsupported assignment target in BbBlock.ops"),
                }
            }
            Stmt::Expr(expr) => Some(Self::Expr(BbExprOp {
                node_index: expr.node_index,
                range: expr.range,
                value: BbExpr::from_expr(*expr.value),
            })),
            Stmt::Delete(delete) => Some(Self::Delete(BbDeleteOp {
                node_index: delete.node_index,
                range: delete.range,
                targets: delete.targets.into_iter().map(BbExpr::from_expr).collect(),
            })),
            Stmt::Pass(_) => None,
            Stmt::FunctionDef(_) => panic!(
                "FunctionDef is not allowed in BbBlock.ops; lower to binding statements first"
            ),
            other => panic!("unsupported statement in BbBlock.ops: {other:?}"),
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
                targets: delete.targets.iter().map(BbExpr::to_expr).collect(),
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
        test: BbExpr,
        then_label: String,
        else_label: String,
    },
    BrTable {
        index: BbExpr,
        targets: Vec<String>,
        default_label: String,
    },
    Raise {
        exc: Option<BbExpr>,
        cause: Option<BbExpr>,
    },
    Ret(Option<BbExpr>),
}
