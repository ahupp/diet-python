use super::bb_ir::{BbClosureLayout, FunctionId};
use super::cfg_ir::{CfgBlock, CfgCallableDef, CfgModule};
use ruff_python_ast::{self as ast, Expr, ExprName, Parameters};
use std::ops::{Deref, DerefMut};

pub(crate) mod cfg;
pub(crate) mod dataflow;
pub(crate) mod exception;
pub(crate) mod pretty;
pub(crate) mod state;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockPyLabel(pub String);

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

pub type RuffBlockPyModule = BlockPyModule<Expr>;
pub type RuffBlockPyCallableDef = BlockPyCallableDef<Expr>;
pub type RuffBlockPyBlock = BlockPyBlock<Expr>;
pub type RuffBlockPyStmt = BlockPyStmt<Expr>;
pub type RuffBlockPyTerm = BlockPyTerm<Expr>;
pub type RuffBlockPyStmtFragment = BlockPyStmtFragment<Expr>;
pub type RuffBlockPyAssign = BlockPyAssign<Expr>;
pub type RuffBlockPyIf = BlockPyIf<Expr>;
pub type RuffBlockPyIfTerm = BlockPyIfTerm<Expr>;
pub type RuffBlockPyBranchTable = BlockPyBranchTable<Expr>;
pub type RuffBlockPyRaise = BlockPyRaise<Expr>;
pub const ENTRY_BLOCK_LABEL: &str = "start";

pub type BlockPyModule<E = BlockPyExpr> = CfgModule<BlockPyCallableDef<E>>;

#[derive(Debug, Clone)]
pub struct BlockPyCallableDef<E = BlockPyExpr> {
    pub cfg: CfgCallableDef<FunctionId, BlockPyFunctionKind, Parameters, BlockPyBlock<E>>,
    pub doc: Option<E>,
    pub closure_layout: Option<BbClosureLayout>,
    pub local_cell_slots: Vec<String>,
}

impl<E> Deref for BlockPyCallableDef<E> {
    type Target = CfgCallableDef<FunctionId, BlockPyFunctionKind, Parameters, BlockPyBlock<E>>;

    fn deref(&self) -> &Self::Target {
        &self.cfg
    }
}

impl<E> DerefMut for BlockPyCallableDef<E> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cfg
    }
}

impl<E> BlockPyCallableDef<E> {
    pub fn entry_label(&self) -> &str {
        self.blocks
            .first()
            .map(|block| block.label.as_str())
            .expect("BlockPyCallableDef should have at least one block")
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockPyFunctionKind {
    Function,
    Coroutine,
    Generator,
    AsyncGenerator,
}

#[derive(Debug, Clone, Default)]
pub struct BlockPyBlockMeta {
    pub exc_param: Option<String>,
}

pub type BlockPyBlock<E = BlockPyExpr> =
    CfgBlock<BlockPyLabel, BlockPyStmt<E>, BlockPyTerm<E>, BlockPyBlockMeta>;

pub fn assert_blockpy_block_normalized<E: std::fmt::Debug>(block: &BlockPyBlock<E>) {
    for stmt in &block.body {
        stmt.assert_normalized();
    }
}

#[derive(Debug, Clone)]
pub struct BlockPyStmtFragment<E = BlockPyExpr> {
    pub body: Vec<BlockPyStmt<E>>,
    pub term: Option<BlockPyTerm<E>>,
}

impl<E: std::fmt::Debug> BlockPyStmtFragment<E> {
    pub fn assert_normalized(&self) {
        for stmt in &self.body {
            stmt.assert_normalized();
        }
    }
}

impl<E: Clone + std::fmt::Debug> BlockPyStmtFragment<E> {
    pub fn from_stmts(stmts: Vec<BlockPyStmt<E>>) -> Self {
        Self::with_term(stmts, None)
    }

    pub fn with_term(body: Vec<BlockPyStmt<E>>, term: impl Into<Option<BlockPyTerm<E>>>) -> Self {
        let fragment = BlockPyStmtFragment {
            body,
            term: term.into(),
        };
        fragment.assert_normalized();
        fragment
    }

    pub fn jump(target: BlockPyLabel) -> Self {
        Self::with_term(Vec::new(), Some(BlockPyTerm::Jump(target)))
    }
}

#[derive(Debug, Clone)]
pub struct BlockPyStmtFragmentBuilder<E = BlockPyExpr> {
    body: Vec<BlockPyStmt<E>>,
    term: Option<BlockPyTerm<E>>,
}

impl<E: Clone + std::fmt::Debug> BlockPyStmtFragmentBuilder<E> {
    pub fn new() -> Self {
        Self {
            body: Vec::new(),
            term: None,
        }
    }

    pub fn push_stmt(&mut self, stmt: BlockPyStmt<E>) {
        assert!(
            self.term.is_none(),
            "cannot append BlockPyStmt after stmt-fragment terminator"
        );
        stmt.assert_normalized();
        self.body.push(stmt);
    }

    pub fn extend<I>(&mut self, stmts: I)
    where
        I: IntoIterator<Item = BlockPyStmt<E>>,
    {
        for stmt in stmts {
            self.push_stmt(stmt);
        }
    }

    pub fn set_term(&mut self, term: BlockPyTerm<E>) {
        assert!(
            self.term.is_none(),
            "cannot replace existing stmt-fragment terminator"
        );
        self.term = Some(term);
    }

    pub fn finish(self) -> BlockPyStmtFragment<E> {
        let fragment = BlockPyStmtFragment {
            body: self.body,
            term: self.term,
        };
        fragment.assert_normalized();
        fragment
    }
}

#[derive(Debug, Clone)]
pub struct BlockPyBlockBuilder<E = BlockPyExpr> {
    label: BlockPyLabel,
    meta: BlockPyBlockMeta,
    fragment: BlockPyStmtFragmentBuilder<E>,
}

impl<E: Clone + std::fmt::Debug> BlockPyBlockBuilder<E> {
    pub fn new(label: BlockPyLabel) -> Self {
        Self {
            label,
            meta: BlockPyBlockMeta::default(),
            fragment: BlockPyStmtFragmentBuilder::new(),
        }
    }

    pub fn with_exc_param(mut self, exc_param: Option<String>) -> Self {
        self.meta.exc_param = exc_param;
        self
    }

    pub fn push_stmt(&mut self, stmt: BlockPyStmt<E>) {
        self.fragment.push_stmt(stmt);
    }

    pub fn extend<I>(&mut self, stmts: I)
    where
        I: IntoIterator<Item = BlockPyStmt<E>>,
    {
        self.fragment.extend(stmts);
    }

    pub fn set_term(&mut self, term: BlockPyTerm<E>) {
        self.fragment.set_term(term);
    }

    pub fn finish(self, fallthrough_target: Option<BlockPyLabel>) -> BlockPyBlock<E> {
        let fragment = self.fragment.finish();
        let block = BlockPyBlock {
            label: self.label,
            body: fragment.body,
            term: fragment.term.unwrap_or_else(|| match fallthrough_target {
                Some(target) => BlockPyTerm::Jump(target),
                None => BlockPyTerm::Return(None),
            }),
            meta: self.meta,
        };
        assert_blockpy_block_normalized(&block);
        block
    }
}

#[derive(Debug, Clone)]
pub enum BlockPyStmt<E = BlockPyExpr> {
    Pass,
    Assign(BlockPyAssign<E>),
    Expr(E),
    Delete(BlockPyDelete),
    If(BlockPyIf<E>),
}

impl<E: std::fmt::Debug> BlockPyStmt<E> {
    pub fn assert_normalized(&self) {
        if let Self::If(if_stmt) = self {
            if_stmt.body.assert_normalized();
            if_stmt.orelse.assert_normalized();
        }
    }
}

#[derive(Debug, Clone)]
pub enum BlockPyTerm<E = BlockPyExpr> {
    Jump(BlockPyLabel),
    IfTerm(BlockPyIfTerm<E>),
    BranchTable(BlockPyBranchTable<E>),
    Raise(BlockPyRaise<E>),
    TryJump(BlockPyTryJump),
    Return(Option<E>),
}

#[derive(Debug, Clone)]
pub struct BlockPyAssign<E = BlockPyExpr> {
    pub target: ExprName,
    pub value: E,
}

#[derive(Debug, Clone)]
pub struct BlockPyDelete {
    pub target: ExprName,
}

#[derive(Debug, Clone)]
pub struct BlockPyIf<E = BlockPyExpr> {
    pub test: E,
    pub body: BlockPyStmtFragment<E>,
    pub orelse: BlockPyStmtFragment<E>,
}

#[derive(Debug, Clone)]
pub struct BlockPyIfTerm<E = BlockPyExpr> {
    pub test: E,
    pub then_label: BlockPyLabel,
    pub else_label: BlockPyLabel,
}

#[derive(Debug, Clone)]
pub struct BlockPyBranchTable<E = BlockPyExpr> {
    pub index: E,
    pub targets: Vec<BlockPyLabel>,
    pub default_label: BlockPyLabel,
}

#[derive(Debug, Clone)]
pub struct BlockPyRaise<E = BlockPyExpr> {
    pub exc: Option<E>,
}

#[derive(Debug, Clone)]
pub struct BlockPyTryJump {
    pub body_label: BlockPyLabel,
    pub except_label: BlockPyLabel,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_builder_sets_explicit_term() {
        let mut block: BlockPyBlockBuilder<BlockPyExpr> =
            BlockPyBlockBuilder::new(BlockPyLabel::from("start"));
        block.push_stmt(BlockPyStmt::Pass);
        block.set_term(BlockPyTerm::Jump(BlockPyLabel::from("after")));
        let block = block.finish(None);

        assert_eq!(block.body.len(), 1);
        assert!(matches!(block.body[0], BlockPyStmt::Pass));
        assert!(matches!(block.term, BlockPyTerm::Jump(_)));
    }

    #[test]
    fn stmt_fragment_can_carry_optional_term() {
        let fragment: BlockPyStmtFragment<BlockPyExpr> = BlockPyStmtFragment::with_term(
            vec![BlockPyStmt::Pass],
            Some(BlockPyTerm::Return(None)),
        );

        assert_eq!(fragment.body.len(), 1);
        assert!(matches!(fragment.body[0], BlockPyStmt::Pass));
        assert!(matches!(fragment.term, Some(BlockPyTerm::Return(None))));
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
