use super::bb_ir::{BbClosureLayout, BindingTarget};
use ruff_python_ast::{self as ast, Expr, ExprName, Parameters};

pub(crate) mod cfg;
pub(crate) mod dataflow;
pub(crate) mod exception;
pub(crate) mod export;
pub(crate) mod pretty;
pub(crate) mod state;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockPyLabel(pub String);

#[derive(Debug, Clone)]
pub struct BlockPyModule {
    pub functions: Vec<BlockPyFunction>,
    pub module_init: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BlockPyFunction {
    pub bind_name: String,
    pub display_name: String,
    pub qualname: String,
    pub binding_target: BindingTarget,
    pub kind: BlockPyFunctionKind,
    pub params: Parameters,
    pub entry_liveins: Vec<String>,
    pub closure_layout: Option<BbClosureLayout>,
    pub local_cell_slots: Vec<String>,
    pub blocks: Vec<BlockPyBlock>,
}

impl BlockPyFunction {
    pub fn entry_label(&self) -> &str {
        self.blocks
            .first()
            .map(|block| block.label.as_str())
            .expect("BlockPyFunction should have at least one block")
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockPyFunctionKind {
    Function,
    Coroutine,
    Generator,
    AsyncGenerator,
}

#[derive(Debug, Clone)]
pub struct BlockPyBlock {
    pub label: BlockPyLabel,
    pub exc_param: Option<String>,
    pub body: Vec<BlockPyStmt>,
    pub term: BlockPyTerm,
}

#[derive(Debug, Clone)]
pub enum BlockPyExpr {
    BoolOp(ast::ExprBoolOp),
    Named(ast::ExprNamed),
    BinOp(ast::ExprBinOp),
    UnaryOp(ast::ExprUnaryOp),
    Lambda(ast::ExprLambda),
    If(ast::ExprIf),
    Dict(ast::ExprDict),
    Set(ast::ExprSet),
    ListComp(ast::ExprListComp),
    SetComp(ast::ExprSetComp),
    DictComp(ast::ExprDictComp),
    Generator(ast::ExprGenerator),
    Await(ast::ExprAwait),
    Yield(ast::ExprYield),
    YieldFrom(ast::ExprYieldFrom),
    Compare(ast::ExprCompare),
    Call(ast::ExprCall),
    FString(ast::ExprFString),
    TString(ast::ExprTString),
    StringLiteral(ast::ExprStringLiteral),
    BytesLiteral(ast::ExprBytesLiteral),
    NumberLiteral(ast::ExprNumberLiteral),
    BooleanLiteral(ast::ExprBooleanLiteral),
    NoneLiteral(ast::ExprNoneLiteral),
    EllipsisLiteral(ast::ExprEllipsisLiteral),
    Attribute(ast::ExprAttribute),
    Subscript(ast::ExprSubscript),
    Starred(ast::ExprStarred),
    Name(ast::ExprName),
    List(ast::ExprList),
    Tuple(ast::ExprTuple),
    Slice(ast::ExprSlice),
}

#[derive(Debug, Clone)]
pub enum BlockPyStmt {
    Pass,
    Assign(BlockPyAssign),
    Expr(BlockPyExpr),
    Delete(BlockPyDelete),
    FunctionDef(ast::StmtFunctionDef),
    If(BlockPyIf),
    BranchTable(BlockPyBranchTable),
    Jump(BlockPyLabel),
    Return(Option<BlockPyExpr>),
    Raise(BlockPyRaise),
    TryJump(BlockPyTryJump),
}

#[derive(Debug, Clone)]
pub enum BlockPyTerm {
    Jump(BlockPyLabel),
    IfTerm(BlockPyIfTerm),
    BranchTable(BlockPyBranchTable),
    Raise(BlockPyRaise),
    TryJump(BlockPyTryJump),
    Return(Option<BlockPyExpr>),
}

#[derive(Debug, Clone)]
pub struct BlockPyAssign {
    pub target: ExprName,
    pub value: BlockPyExpr,
}

#[derive(Debug, Clone)]
pub struct BlockPyDelete {
    pub target: ExprName,
}

#[derive(Debug, Clone)]
pub struct BlockPyIf {
    pub test: BlockPyExpr,
    pub body: Vec<BlockPyStmt>,
    pub orelse: Vec<BlockPyStmt>,
}

#[derive(Debug, Clone)]
pub struct BlockPyIfTerm {
    pub test: BlockPyExpr,
    pub then_label: BlockPyLabel,
    pub else_label: BlockPyLabel,
}

#[derive(Debug, Clone)]
pub struct BlockPyBranchTable {
    pub index: BlockPyExpr,
    pub targets: Vec<BlockPyLabel>,
    pub default_label: BlockPyLabel,
}

#[derive(Debug, Clone)]
pub struct BlockPyRaise {
    pub exc: Option<BlockPyExpr>,
}

#[derive(Debug, Clone)]
pub struct BlockPyTryJump {
    pub body_label: BlockPyLabel,
    pub except_label: BlockPyLabel,
}

impl BlockPyTerm {
    pub fn from_stmt(stmt: &BlockPyStmt) -> Option<Self> {
        match stmt {
            BlockPyStmt::Jump(target) => Some(Self::Jump(target.clone())),
            BlockPyStmt::BranchTable(branch) => Some(Self::BranchTable(branch.clone())),
            BlockPyStmt::Return(value) => Some(Self::Return(value.clone())),
            BlockPyStmt::Raise(raise_stmt) => Some(Self::Raise(raise_stmt.clone())),
            BlockPyStmt::TryJump(try_jump) => Some(Self::TryJump(try_jump.clone())),
            BlockPyStmt::Pass
            | BlockPyStmt::Assign(_)
            | BlockPyStmt::Expr(_)
            | BlockPyStmt::Delete(_)
            | BlockPyStmt::If(_)
            | BlockPyStmt::FunctionDef(_) => None,
        }
    }
}

impl From<Expr> for BlockPyExpr {
    fn from(value: Expr) -> Self {
        match value {
            Expr::BoolOp(node) => Self::BoolOp(node),
            Expr::Named(node) => Self::Named(node),
            Expr::BinOp(node) => Self::BinOp(node),
            Expr::UnaryOp(node) => Self::UnaryOp(node),
            Expr::Lambda(node) => Self::Lambda(node),
            Expr::If(node) => Self::If(node),
            Expr::Dict(node) => Self::Dict(node),
            Expr::Set(node) => Self::Set(node),
            Expr::ListComp(node) => Self::ListComp(node),
            Expr::SetComp(node) => Self::SetComp(node),
            Expr::DictComp(node) => Self::DictComp(node),
            Expr::Generator(node) => Self::Generator(node),
            Expr::Await(node) => Self::Await(node),
            Expr::Yield(node) => Self::Yield(node),
            Expr::YieldFrom(node) => Self::YieldFrom(node),
            Expr::Compare(node) => Self::Compare(node),
            Expr::Call(node) => Self::Call(node),
            Expr::FString(node) => Self::FString(node),
            Expr::TString(node) => Self::TString(node),
            Expr::StringLiteral(node) => Self::StringLiteral(node),
            Expr::BytesLiteral(node) => Self::BytesLiteral(node),
            Expr::NumberLiteral(node) => Self::NumberLiteral(node),
            Expr::BooleanLiteral(node) => Self::BooleanLiteral(node),
            Expr::NoneLiteral(node) => Self::NoneLiteral(node),
            Expr::EllipsisLiteral(node) => Self::EllipsisLiteral(node),
            Expr::Attribute(node) => Self::Attribute(node),
            Expr::Subscript(node) => Self::Subscript(node),
            Expr::Starred(node) => Self::Starred(node),
            Expr::Name(node) => Self::Name(node),
            Expr::List(node) => Self::List(node),
            Expr::Tuple(node) => Self::Tuple(node),
            Expr::Slice(node) => Self::Slice(node),
            Expr::IpyEscapeCommand(_) => panic!("IpyEscapeCommand should not reach BlockPy"),
        }
    }
}

impl From<BlockPyExpr> for Expr {
    fn from(value: BlockPyExpr) -> Self {
        match value {
            BlockPyExpr::BoolOp(node) => Expr::BoolOp(node),
            BlockPyExpr::Named(node) => Expr::Named(node),
            BlockPyExpr::BinOp(node) => Expr::BinOp(node),
            BlockPyExpr::UnaryOp(node) => Expr::UnaryOp(node),
            BlockPyExpr::Lambda(node) => Expr::Lambda(node),
            BlockPyExpr::If(node) => Expr::If(node),
            BlockPyExpr::Dict(node) => Expr::Dict(node),
            BlockPyExpr::Set(node) => Expr::Set(node),
            BlockPyExpr::ListComp(node) => Expr::ListComp(node),
            BlockPyExpr::SetComp(node) => Expr::SetComp(node),
            BlockPyExpr::DictComp(node) => Expr::DictComp(node),
            BlockPyExpr::Generator(node) => Expr::Generator(node),
            BlockPyExpr::Await(node) => Expr::Await(node),
            BlockPyExpr::Yield(node) => Expr::Yield(node),
            BlockPyExpr::YieldFrom(node) => Expr::YieldFrom(node),
            BlockPyExpr::Compare(node) => Expr::Compare(node),
            BlockPyExpr::Call(node) => Expr::Call(node),
            BlockPyExpr::FString(node) => Expr::FString(node),
            BlockPyExpr::TString(node) => Expr::TString(node),
            BlockPyExpr::StringLiteral(node) => Expr::StringLiteral(node),
            BlockPyExpr::BytesLiteral(node) => Expr::BytesLiteral(node),
            BlockPyExpr::NumberLiteral(node) => Expr::NumberLiteral(node),
            BlockPyExpr::BooleanLiteral(node) => Expr::BooleanLiteral(node),
            BlockPyExpr::NoneLiteral(node) => Expr::NoneLiteral(node),
            BlockPyExpr::EllipsisLiteral(node) => Expr::EllipsisLiteral(node),
            BlockPyExpr::Attribute(node) => Expr::Attribute(node),
            BlockPyExpr::Subscript(node) => Expr::Subscript(node),
            BlockPyExpr::Starred(node) => Expr::Starred(node),
            BlockPyExpr::Name(node) => Expr::Name(node),
            BlockPyExpr::List(node) => Expr::List(node),
            BlockPyExpr::Tuple(node) => Expr::Tuple(node),
            BlockPyExpr::Slice(node) => Expr::Slice(node),
        }
    }
}

impl BlockPyExpr {
    pub fn to_expr(&self) -> Expr {
        self.clone().into()
    }

    pub fn rewrite_mut(&mut self, f: impl FnOnce(&mut Expr)) {
        let mut expr = self.to_expr();
        f(&mut expr);
        *self = expr.into();
    }
}

impl From<String> for BlockPyLabel {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for BlockPyLabel {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl BlockPyLabel {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
