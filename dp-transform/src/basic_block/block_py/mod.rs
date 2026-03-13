use super::bb_ir::{BbClosureLayout, FunctionId};
use super::cfg_ir::{CfgBlock, CfgCallableDef, CfgModule};
use ruff_python_ast::{self as ast, Expr, ExprName, Parameters};
use std::ops::{Deref, DerefMut};

pub(crate) mod cfg;
pub(crate) mod core_lower;
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

#[derive(Debug, Clone)]
pub enum CoreBlockPyExpr {
    Call(ast::ExprCall),
    StringLiteral(ast::ExprStringLiteral),
    BytesLiteral(ast::ExprBytesLiteral),
    NumberLiteral(ast::ExprNumberLiteral),
    BooleanLiteral(ast::ExprBooleanLiteral),
    NoneLiteral(ast::ExprNoneLiteral),
    Attribute(ast::ExprAttribute),
    Subscript(ast::ExprSubscript),
    Name(ast::ExprName),
    Raw(Expr),
}

pub type RuffBlockPyModule = BlockPyModule<Expr>;
pub type RuffBlockPyCallableDef = BlockPyCallableDef<Expr>;
pub type RuffBlockPyBlock = BlockPyBlock<Expr>;
pub type RuffBlockPyStmt = BlockPyStmt<Expr>;
pub type RuffBlockPyTerm = BlockPyTerm<Expr>;
pub type RuffBlockPyStmtFragment = BlockPyStmtFragment<Expr>;
pub type RuffBlockPyAssign = BlockPyAssign<Expr>;
pub type RuffBlockPyIf = BlockPyStructuredIf<Expr>;
pub type RuffBlockPyIfTerm = BlockPyIfTerm<Expr>;
pub type RuffBlockPyBranchTable = BlockPyBranchTable<Expr>;
pub type RuffBlockPyRaise = BlockPyRaise<Expr>;
pub type SemanticBlockPyModule = BlockPyModule<BlockPyExpr>;
pub type SemanticBlockPyCallableDef = BlockPyCallableDef<BlockPyExpr>;
pub type SemanticBlockPyBlock = BlockPyBlock<BlockPyExpr>;
pub type SemanticBlockPyStmt = BlockPyStmt<BlockPyExpr>;
pub type SemanticBlockPyTerm = BlockPyTerm<BlockPyExpr>;
pub type SemanticBlockPyStmtFragment = BlockPyStmtFragment<BlockPyExpr>;
pub type SemanticBlockPyAssign = BlockPyAssign<BlockPyExpr>;
pub type SemanticBlockPyIf = BlockPyStructuredIf<BlockPyExpr>;
pub type SemanticBlockPyIfTerm = BlockPyIfTerm<BlockPyExpr>;
pub type SemanticBlockPyBranchTable = BlockPyBranchTable<BlockPyExpr>;
pub type SemanticBlockPyRaise = BlockPyRaise<BlockPyExpr>;
pub type CoreBlockPyModule = BlockPyModule<CoreBlockPyExpr>;
pub type CoreBlockPyCallableDef = BlockPyCallableDef<CoreBlockPyExpr>;
pub type CoreBlockPyBlock = BlockPyBlock<CoreBlockPyExpr>;
pub type CoreBlockPyStmt = BlockPyStmt<CoreBlockPyExpr>;
pub type CoreBlockPyTerm = BlockPyTerm<CoreBlockPyExpr>;
pub type CoreBlockPyStmtFragment = BlockPyStmtFragment<CoreBlockPyExpr>;
pub type CoreBlockPyAssign = BlockPyAssign<CoreBlockPyExpr>;
pub type CoreBlockPyIf = BlockPyStructuredIf<CoreBlockPyExpr>;
pub type CoreBlockPyIfTerm = BlockPyIfTerm<CoreBlockPyExpr>;
pub type CoreBlockPyBranchTable = BlockPyBranchTable<CoreBlockPyExpr>;
pub type CoreBlockPyRaise = BlockPyRaise<CoreBlockPyExpr>;
pub const ENTRY_BLOCK_LABEL: &str = "start";

#[derive(Debug, Clone)]
pub struct BlockPyCallableDef<E = BlockPyExpr, B = BlockPyBlock<E>> {
    pub cfg: CfgCallableDef<FunctionId, BlockPyFunctionKind, Parameters, B>,
    pub doc: Option<E>,
    pub closure_layout: Option<BbClosureLayout>,
    pub local_cell_slots: Vec<String>,
}

impl<E, B> Deref for BlockPyCallableDef<E, B> {
    type Target = CfgCallableDef<FunctionId, BlockPyFunctionKind, Parameters, B>;

    fn deref(&self) -> &Self::Target {
        &self.cfg
    }
}

impl<E, B> DerefMut for BlockPyCallableDef<E, B> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cfg
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

pub type BlockPyCfgBlock<S, T> = CfgBlock<BlockPyLabel, S, T, BlockPyBlockMeta>;
pub type BlockPyBlock<E = BlockPyExpr> = BlockPyCfgBlock<BlockPyStmt<E>, BlockPyTerm<E>>;
pub type BlockPyModuleWith<S, T, E = BlockPyExpr> =
    CfgModule<BlockPyCallableDef<E, BlockPyCfgBlock<S, T>>>;
pub type BlockPyModule<E = BlockPyExpr> = BlockPyModuleWith<BlockPyStmt<E>, BlockPyTerm<E>, E>;
pub type BlockPyStructuredIf<E = BlockPyExpr> = BlockPyIf<E, BlockPyStmt<E>, BlockPyTerm<E>>;

pub trait BlockPyNormalizedStmt {
    fn assert_blockpy_normalized(&self);
}

pub trait BlockPyJumpTerm<L> {
    fn jump_term(target: L) -> Self;
}

pub trait BlockPyFallthroughTerm<L>: BlockPyJumpTerm<L> {
    fn implicit_function_return() -> Self;
}

pub fn assert_blockpy_block_normalized<S: BlockPyNormalizedStmt, T>(block: &BlockPyCfgBlock<S, T>) {
    for stmt in &block.body {
        stmt.assert_blockpy_normalized();
    }
}

#[derive(Debug, Clone)]
pub struct BlockPyCfgFragment<S, T> {
    pub body: Vec<S>,
    pub term: Option<T>,
}

pub type BlockPyStmtFragment<E = BlockPyExpr> = BlockPyCfgFragment<BlockPyStmt<E>, BlockPyTerm<E>>;

impl<S: BlockPyNormalizedStmt, T> BlockPyCfgFragment<S, T> {
    pub fn assert_normalized(&self) {
        for stmt in &self.body {
            stmt.assert_blockpy_normalized();
        }
    }
}

impl<S: BlockPyNormalizedStmt, T> BlockPyCfgFragment<S, T> {
    pub fn from_stmts(stmts: Vec<S>) -> Self {
        Self::with_term(stmts, None)
    }

    pub fn with_term(body: Vec<S>, term: impl Into<Option<T>>) -> Self {
        let fragment = BlockPyCfgFragment {
            body,
            term: term.into(),
        };
        fragment.assert_normalized();
        fragment
    }
}

impl<S: BlockPyNormalizedStmt, T: BlockPyJumpTerm<BlockPyLabel>> BlockPyCfgFragment<S, T> {
    pub fn jump(target: BlockPyLabel) -> Self {
        Self::with_term(Vec::new(), Some(T::jump_term(target)))
    }
}

#[derive(Debug, Clone)]
pub struct BlockPyCfgFragmentBuilder<S, T> {
    body: Vec<S>,
    term: Option<T>,
}

pub type BlockPyStmtFragmentBuilder<E = BlockPyExpr> =
    BlockPyCfgFragmentBuilder<BlockPyStmt<E>, BlockPyTerm<E>>;

impl<S: BlockPyNormalizedStmt, T> BlockPyCfgFragmentBuilder<S, T> {
    pub fn new() -> Self {
        Self {
            body: Vec::new(),
            term: None,
        }
    }

    pub fn push_stmt(&mut self, stmt: S) {
        assert!(
            self.term.is_none(),
            "cannot append BlockPyStmt after stmt-fragment terminator"
        );
        stmt.assert_blockpy_normalized();
        self.body.push(stmt);
    }

    pub fn extend<I>(&mut self, stmts: I)
    where
        I: IntoIterator<Item = S>,
    {
        for stmt in stmts {
            self.push_stmt(stmt);
        }
    }

    pub fn set_term(&mut self, term: T) {
        assert!(
            self.term.is_none(),
            "cannot replace existing stmt-fragment terminator"
        );
        self.term = Some(term);
    }

    pub fn finish(self) -> BlockPyCfgFragment<S, T> {
        let fragment = BlockPyCfgFragment {
            body: self.body,
            term: self.term,
        };
        fragment.assert_normalized();
        fragment
    }
}

#[derive(Debug, Clone)]
pub struct BlockPyCfgBlockBuilder<S, T> {
    label: BlockPyLabel,
    meta: BlockPyBlockMeta,
    fragment: BlockPyCfgFragmentBuilder<S, T>,
}

pub type BlockPyBlockBuilder<E = BlockPyExpr> =
    BlockPyCfgBlockBuilder<BlockPyStmt<E>, BlockPyTerm<E>>;

impl<S: BlockPyNormalizedStmt, T: BlockPyFallthroughTerm<BlockPyLabel>>
    BlockPyCfgBlockBuilder<S, T>
{
    pub fn new(label: BlockPyLabel) -> Self {
        Self {
            label,
            meta: BlockPyBlockMeta::default(),
            fragment: BlockPyCfgFragmentBuilder::new(),
        }
    }

    pub fn with_exc_param(mut self, exc_param: Option<String>) -> Self {
        self.meta.exc_param = exc_param;
        self
    }

    pub fn push_stmt(&mut self, stmt: S) {
        self.fragment.push_stmt(stmt);
    }

    pub fn extend<I>(&mut self, stmts: I)
    where
        I: IntoIterator<Item = S>,
    {
        self.fragment.extend(stmts);
    }

    pub fn set_term(&mut self, term: T) {
        self.fragment.set_term(term);
    }

    pub fn finish(self, fallthrough_target: Option<BlockPyLabel>) -> BlockPyCfgBlock<S, T> {
        let fragment = self.fragment.finish();
        let block = BlockPyCfgBlock {
            label: self.label,
            body: fragment.body,
            term: fragment.term.unwrap_or_else(|| match fallthrough_target {
                Some(target) => T::jump_term(target),
                None => T::implicit_function_return(),
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
    If(BlockPyStructuredIf<E>),
}

impl<E: std::fmt::Debug> BlockPyStmt<E> {
    pub fn assert_normalized(&self) {
        if let Self::If(if_stmt) = self {
            if_stmt.body.assert_normalized();
            if_stmt.orelse.assert_normalized();
        }
    }
}

impl<E: std::fmt::Debug> BlockPyNormalizedStmt for BlockPyStmt<E> {
    fn assert_blockpy_normalized(&self) {
        self.assert_normalized();
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
pub struct BlockPyIf<E = BlockPyExpr, S = BlockPyStmt<E>, T = BlockPyTerm<E>> {
    pub test: E,
    pub body: BlockPyCfgFragment<S, T>,
    pub orelse: BlockPyCfgFragment<S, T>,
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

impl<E> BlockPyJumpTerm<BlockPyLabel> for BlockPyTerm<E> {
    fn jump_term(target: BlockPyLabel) -> Self {
        Self::Jump(target)
    }
}

impl<E> BlockPyFallthroughTerm<BlockPyLabel> for BlockPyTerm<E> {
    fn implicit_function_return() -> Self {
        Self::Return(None)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::py_expr;

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

    #[test]
    fn core_blockpy_expr_wraps_and_rewrites_expr() {
        let mut expr = CoreBlockPyExpr::from(py_expr!("x"));
        expr.rewrite_mut(|expr| *expr = py_expr!("y"));

        let Expr::Name(name) = expr.to_expr() else {
            panic!("expected name expr after rewrite");
        };
        assert_eq!(name.id.as_str(), "y");
    }

    #[test]
    fn core_blockpy_expr_uses_reduced_variants_for_simple_shapes() {
        assert!(matches!(
            CoreBlockPyExpr::from(py_expr!("x")),
            CoreBlockPyExpr::Name(_)
        ));
        assert!(matches!(
            CoreBlockPyExpr::from(py_expr!("1")),
            CoreBlockPyExpr::NumberLiteral(_)
        ));
        assert!(matches!(
            CoreBlockPyExpr::from(py_expr!("f(x)")),
            CoreBlockPyExpr::Call(_)
        ));
        assert!(matches!(
            CoreBlockPyExpr::from(py_expr!("x + 1")),
            CoreBlockPyExpr::Raw(_)
        ));
    }

    #[test]
    fn core_blockpy_expr_uses_reduced_variants_for_attribute_and_subscript() {
        assert!(matches!(
            CoreBlockPyExpr::from(py_expr!("obj.attr")),
            CoreBlockPyExpr::Attribute(_)
        ));
        assert!(matches!(
            CoreBlockPyExpr::from(py_expr!("obj[idx]")),
            CoreBlockPyExpr::Subscript(_)
        ));
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

impl From<Expr> for CoreBlockPyExpr {
    fn from(value: Expr) -> Self {
        match value {
            Expr::Call(node) => Self::Call(node),
            Expr::StringLiteral(node) => Self::StringLiteral(node),
            Expr::BytesLiteral(node) => Self::BytesLiteral(node),
            Expr::NumberLiteral(node) => Self::NumberLiteral(node),
            Expr::BooleanLiteral(node) => Self::BooleanLiteral(node),
            Expr::NoneLiteral(node) => Self::NoneLiteral(node),
            Expr::Attribute(node) => Self::Attribute(node),
            Expr::Subscript(node) => Self::Subscript(node),
            Expr::Name(node) => Self::Name(node),
            other => Self::Raw(other),
        }
    }
}

impl From<BlockPyExpr> for CoreBlockPyExpr {
    fn from(value: BlockPyExpr) -> Self {
        Expr::from(value).into()
    }
}

impl From<CoreBlockPyExpr> for Expr {
    fn from(value: CoreBlockPyExpr) -> Self {
        match value {
            CoreBlockPyExpr::Call(node) => Expr::Call(node),
            CoreBlockPyExpr::StringLiteral(node) => Expr::StringLiteral(node),
            CoreBlockPyExpr::BytesLiteral(node) => Expr::BytesLiteral(node),
            CoreBlockPyExpr::NumberLiteral(node) => Expr::NumberLiteral(node),
            CoreBlockPyExpr::BooleanLiteral(node) => Expr::BooleanLiteral(node),
            CoreBlockPyExpr::NoneLiteral(node) => Expr::NoneLiteral(node),
            CoreBlockPyExpr::Attribute(node) => Expr::Attribute(node),
            CoreBlockPyExpr::Subscript(node) => Expr::Subscript(node),
            CoreBlockPyExpr::Name(node) => Expr::Name(node),
            CoreBlockPyExpr::Raw(expr) => expr,
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

impl CoreBlockPyExpr {
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

impl AsRef<str> for BlockPyLabel {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}
