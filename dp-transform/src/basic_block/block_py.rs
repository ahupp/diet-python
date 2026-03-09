use super::bb_ir::BindingTarget;
use ruff_python_ast::{self as ast, Expr, ExprName, Parameters};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockPyLabel(pub String);

#[derive(Debug, Clone)]
pub struct BlockPyModule {
    pub prelude: Vec<BlockPyStmt>,
    pub functions: Vec<BlockPyFunction>,
    pub module_init: Option<String>,
}

#[derive(Debug, Clone)]
pub struct BlockPyFunction {
    pub bind_name: String,
    pub qualname: String,
    pub binding_target: BindingTarget,
    pub kind: BlockPyFunctionKind,
    pub generator: Option<BlockPyGeneratorInfo>,
    pub params: Parameters,
    pub blocks: Vec<BlockPyBlock>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockPyFunctionKind {
    Function,
    Coroutine,
    Generator,
    AsyncGenerator,
}

#[derive(Debug, Clone)]
pub struct BlockPyGeneratorInfo {
    pub closure_state: bool,
    pub dispatch_entry_label: Option<BlockPyLabel>,
    pub resume_order: Vec<BlockPyLabel>,
    pub yield_sites: Vec<BlockPyGeneratorYieldSite>,
    pub done_block_label: Option<BlockPyLabel>,
    pub invalid_block_label: Option<BlockPyLabel>,
    pub uncaught_block_label: Option<BlockPyLabel>,
    pub uncaught_set_done_label: Option<BlockPyLabel>,
    pub uncaught_raise_label: Option<BlockPyLabel>,
    pub uncaught_exc_name: Option<String>,
    pub dispatch_only_labels: Vec<BlockPyLabel>,
    pub throw_passthrough_labels: Vec<BlockPyLabel>,
}

#[derive(Debug, Clone)]
pub struct BlockPyGeneratorYieldSite {
    pub yield_label: BlockPyLabel,
    pub resume_label: BlockPyLabel,
}

#[derive(Debug, Clone)]
pub struct BlockPyBlock {
    pub label: BlockPyLabel,
    pub body: Vec<BlockPyStmt>,
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
    Try(BlockPyTry),
    LegacyTryJump(BlockPyLegacyTryJump),
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
    pub body: Vec<BlockPyBlock>,
    pub orelse: Vec<BlockPyBlock>,
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
pub struct BlockPyTry {
    pub body: Vec<BlockPyBlock>,
    pub handlers: Vec<BlockPyExceptHandler>,
    pub orelse: Vec<BlockPyBlock>,
    pub finalbody: Vec<BlockPyBlock>,
}

#[derive(Debug, Clone)]
pub struct BlockPyLegacyTryJump {
    pub body_label: BlockPyLabel,
    pub except_label: BlockPyLabel,
    pub except_exc_name: Option<String>,
    pub body_region_labels: Vec<BlockPyLabel>,
    pub except_region_labels: Vec<BlockPyLabel>,
    pub finally_label: Option<BlockPyLabel>,
    pub finally_exc_name: Option<String>,
    pub finally_region_labels: Vec<BlockPyLabel>,
    pub finally_fallthrough_label: Option<BlockPyLabel>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockPyExceptHandlerKind {
    Except,
    ExceptStar,
}

#[derive(Debug, Clone)]
pub struct BlockPyExceptHandler {
    pub kind: BlockPyExceptHandlerKind,
    pub type_: Option<BlockPyExpr>,
    pub name: Option<String>,
    pub body: Vec<BlockPyBlock>,
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
