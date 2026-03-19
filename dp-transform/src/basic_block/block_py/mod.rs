use self::param_specs::ParamSpec;
use crate::basic_block::block_py::dataflow::compute_block_params_blockpy;
use crate::basic_block::block_py::state::collect_state_vars;
pub use ruff_python_ast::Expr;
use ruff_python_ast::{self as ast, ExprName};
use std::borrow::Borrow;
use std::collections::HashSet;
use std::fmt;
use std::ops::Deref;

pub(crate) mod cfg;
pub(crate) mod dataflow;
pub(crate) mod exception;
pub(crate) mod param_specs;
pub(crate) mod pretty;
pub(crate) mod state;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockPyLabel(pub String);

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureLayout {
    pub freevars: Vec<ClosureSlot>,
    pub cellvars: Vec<ClosureSlot>,
    pub runtime_cells: Vec<ClosureSlot>,
}

impl ClosureLayout {
    pub fn ambient_storage_names(&self) -> Vec<String> {
        self.freevars
            .iter()
            .chain(self.cellvars.iter())
            .chain(self.runtime_cells.iter())
            .filter(|slot| matches!(slot.init, ClosureInit::InheritedCapture))
            .map(|slot| slot.storage_name.clone())
            .collect()
    }

    pub fn local_cell_storage_names(&self) -> Vec<String> {
        self.cellvars
            .iter()
            .chain(self.runtime_cells.iter())
            .map(|slot| slot.storage_name.clone())
            .collect()
    }
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockPyFunctionKind {
    Function,
    Coroutine,
    Generator,
    AsyncGenerator,
}

#[derive(Debug, Clone)]
pub struct CfgBlock<S, T, M = ()> {
    pub label: BlockPyLabel,
    pub body: Vec<S>,
    pub term: T,
    pub meta: M,
}

impl<S, T, M> CfgBlock<S, T, M> {
    pub fn label_str(&self) -> &str {
        self.label.as_str()
    }
}

#[derive(Debug, Clone, Default)]
pub struct CfgModule<F> {
    pub callable_defs: Vec<F>,
}

impl<F> CfgModule<F> {
    pub fn map_callable_defs<G>(&self, mut f: impl FnMut(&F) -> G) -> CfgModule<G> {
        CfgModule {
            callable_defs: self.callable_defs.iter().map(&mut f).collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum CoreBlockPyExpr {
    Name(ast::ExprName),
    Literal(CoreBlockPyLiteral),
    Call(CoreBlockPyCall),
    Await(CoreBlockPyAwait),
    Yield(CoreBlockPyYield),
    YieldFrom(CoreBlockPyYieldFrom),
}

#[derive(Debug, Clone)]
pub enum CoreBlockPyExprWithoutAwait {
    Name(ast::ExprName),
    Literal(CoreBlockPyLiteral),
    Call(CoreBlockPyCall<CoreBlockPyExprWithoutAwait>),
    Yield(CoreBlockPyYield<CoreBlockPyExprWithoutAwait>),
    YieldFrom(CoreBlockPyYieldFrom<CoreBlockPyExprWithoutAwait>),
}

#[derive(Debug, Clone)]
pub enum CoreBlockPyExprWithoutAwaitOrYield {
    Name(ast::ExprName),
    Literal(CoreBlockPyLiteral),
    Call(CoreBlockPyCall<CoreBlockPyExprWithoutAwaitOrYield>),
}

#[derive(Debug, Clone)]
pub enum CoreBlockPyLiteral {
    StringLiteral(ast::ExprStringLiteral),
    BytesLiteral(ast::ExprBytesLiteral),
    NumberLiteral(ast::ExprNumberLiteral),
    BooleanLiteral(ast::ExprBooleanLiteral),
    NoneLiteral(ast::ExprNoneLiteral),
    EllipsisLiteral(ast::ExprEllipsisLiteral),
}

#[derive(Debug, Clone)]
pub struct CoreBlockPyCall<E = CoreBlockPyExpr> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub func: Box<E>,
    pub args: Vec<CoreBlockPyCallArg<E>>,
    pub keywords: Vec<CoreBlockPyKeywordArg<E>>,
}

#[derive(Debug, Clone)]
pub enum CoreBlockPyCallArg<E = CoreBlockPyExpr> {
    Positional(E),
    Starred(E),
}

#[derive(Debug, Clone)]
pub enum CoreBlockPyKeywordArg<E = CoreBlockPyExpr> {
    Named { arg: ast::Identifier, value: E },
    Starred(E),
}

#[derive(Debug, Clone)]
pub struct CoreBlockPyAwait<E = CoreBlockPyExpr> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub value: Box<E>,
}

#[derive(Debug, Clone)]
pub struct CoreBlockPyYield<E = CoreBlockPyExpr> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub value: Option<Box<E>>,
}

#[derive(Debug, Clone)]
pub struct CoreBlockPyYieldFrom<E = CoreBlockPyExpr> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub value: Box<E>,
}

pub const ENTRY_BLOCK_LABEL: &str = "start";

#[derive(Debug, Clone)]
pub struct BlockPyCallableFacts {
    pub deleted_names: HashSet<String>,
    pub unbound_local_names: HashSet<String>,
    pub outer_scope_names: HashSet<String>,
    pub cell_slots: HashSet<String>,
}

impl Default for BlockPyCallableFacts {
    fn default() -> Self {
        Self {
            deleted_names: HashSet::new(),
            unbound_local_names: HashSet::new(),
            outer_scope_names: HashSet::new(),
            cell_slots: HashSet::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TryRegionPlan {
    pub body_region_labels: Vec<String>,
    pub body_exception_target: String,
    pub cleanup_region_labels: Vec<String>,
    pub cleanup_exception_target: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FunctionName {
    pub bind_name: String,
    pub fn_name: String,
    pub display_name: String,
    pub qualname: String,
}

impl FunctionName {
    pub fn new(
        bind_name: impl Into<String>,
        fn_name: impl Into<String>,
        display_name: impl Into<String>,
        qualname: impl Into<String>,
    ) -> Self {
        Self {
            bind_name: bind_name.into(),
            fn_name: fn_name.into(),
            display_name: display_name.into(),
            qualname: qualname.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BlockPyCallableDef<E = Expr, B = BlockPyBlock<E>> {
    pub function_id: FunctionId,
    pub names: FunctionName,
    pub kind: BlockPyFunctionKind,
    pub params: ParamSpec,
    pub param_defaults: Vec<E>,
    pub blocks: Vec<B>,
    pub doc: Option<String>,
    pub closure_layout: Option<ClosureLayout>,
    pub facts: BlockPyCallableFacts,
    pub try_regions: Vec<TryRegionPlan>,
}

impl<E, B> BlockPyCallableDef<E, B> {
    pub fn lowered_kind(&self) -> &BlockPyFunctionKind {
        &self.kind
    }

    pub fn closure_layout(&self) -> &Option<ClosureLayout> {
        &self.closure_layout
    }

    pub fn local_cell_slots(&self) -> Vec<String> {
        self.closure_layout
            .as_ref()
            .map(ClosureLayout::local_cell_storage_names)
            .unwrap_or_default()
    }

    pub fn entry_block(&self) -> &B {
        self.blocks
            .first()
            .expect("BlockPyCallableDef should have at least one block")
    }
}

impl<E, S, T, M> BlockPyCallableDef<E, CfgBlock<S, T, M>> {
    pub fn entry_label(&self) -> &str {
        self.entry_block().label_str()
    }
}

pub(crate) fn is_internal_entry_livein(name: &str) -> bool {
    matches!(name, "_dp_self" | "_dp_send_value" | "_dp_resume_exc")
}

impl<E> BlockPyCallableDef<E, BlockPyBlock<E>>
where
    E: Clone + Into<Expr>,
{
    pub fn entry_liveins(&self) -> Vec<String> {
        if self.blocks.is_empty() {
            return Vec::new();
        }
        let param_names = self.params.names();
        let state_vars = collect_state_vars(&param_names, &self.blocks);
        let mut block_params = compute_block_params_blockpy(
            &self.blocks,
            &state_vars,
            &super::ruff_to_blockpy::build_try_extra_successors(&self.try_regions),
        );
        for block in &self.blocks {
            let Some(exc_param) = block.meta.exc_param.as_ref() else {
                continue;
            };
            let params = block_params
                .entry(block.label.as_str().to_string())
                .or_default();
            if !params.iter().any(|existing| existing == exc_param) {
                params.push(exc_param.clone());
            }
        }
        block_params
            .get(self.entry_label())
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|name| !is_internal_entry_livein(name))
            .collect()
    }
}

#[derive(Debug, Clone, Default)]
pub struct BlockPyBlockMeta {
    pub exc_param: Option<String>,
}

pub type BlockPyCfgBlock<S, T> = CfgBlock<S, T, BlockPyBlockMeta>;
pub type BlockPyBlock<E = Expr> = BlockPyCfgBlock<BlockPyStmt<E>, BlockPyTerm<E>>;
pub type BlockPyModuleWith<S, T, E = Expr> =
    CfgModule<BlockPyCallableDef<E, BlockPyCfgBlock<S, T>>>;
pub type BlockPyModule<E = Expr> = BlockPyModuleWith<BlockPyStmt<E>, BlockPyTerm<E>, E>;
pub type BlockPyStructuredIf<E = Expr> = BlockPyIf<E, BlockPyStmt<E>, BlockPyTerm<E>>;

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

pub type BlockPyStmtFragment<E = Expr> = BlockPyCfgFragment<BlockPyStmt<E>, BlockPyTerm<E>>;

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

pub type BlockPyStmtFragmentBuilder<E = Expr> =
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

pub type BlockPyBlockBuilder<E = Expr> = BlockPyCfgBlockBuilder<BlockPyStmt<E>, BlockPyTerm<E>>;

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
pub enum BlockPyStmt<E = Expr> {
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
pub enum BlockPyTerm<E = Expr> {
    Jump(BlockPyLabel),
    IfTerm(BlockPyIfTerm<E>),
    BranchTable(BlockPyBranchTable<E>),
    Raise(BlockPyRaise<E>),
    TryJump(BlockPyTryJump),
    Return(Option<E>),
}

#[derive(Debug, Clone)]
pub struct BlockPyAssign<E = Expr> {
    pub target: ExprName,
    pub value: E,
}

#[derive(Debug, Clone)]
pub struct BlockPyDelete {
    pub target: ExprName,
}

#[derive(Debug, Clone)]
pub struct BlockPyIf<E = Expr, S = BlockPyStmt<E>, T = BlockPyTerm<E>> {
    pub test: E,
    pub body: BlockPyCfgFragment<S, T>,
    pub orelse: BlockPyCfgFragment<S, T>,
}

#[derive(Debug, Clone)]
pub struct BlockPyIfTerm<E = Expr> {
    pub test: E,
    pub then_label: BlockPyLabel,
    pub else_label: BlockPyLabel,
}

#[derive(Debug, Clone)]
pub struct BlockPyBranchTable<E = Expr> {
    pub index: E,
    pub targets: Vec<BlockPyLabel>,
    pub default_label: BlockPyLabel,
}

#[derive(Debug, Clone)]
pub struct BlockPyRaise<E = Expr> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::py_expr;

    #[test]
    fn block_builder_sets_explicit_term() {
        let mut block: BlockPyBlockBuilder<Expr> =
            BlockPyBlockBuilder::new(BlockPyLabel::from("start"));
        block.push_stmt(BlockPyStmt::Expr(py_expr!("x")));
        block.set_term(BlockPyTerm::Jump(BlockPyLabel::from("after")));
        let block = block.finish(None);

        assert_eq!(block.body.len(), 1);
        assert!(matches!(block.body[0], BlockPyStmt::Expr(_)));
        assert!(matches!(block.term, BlockPyTerm::Jump(_)));
    }

    #[test]
    fn stmt_fragment_can_carry_optional_term() {
        let fragment: BlockPyStmtFragment<Expr> = BlockPyStmtFragment::with_term(
            vec![BlockPyStmt::Expr(py_expr!("x"))],
            Some(BlockPyTerm::Return(None)),
        );

        assert_eq!(fragment.body.len(), 1);
        assert!(matches!(fragment.body[0], BlockPyStmt::Expr(_)));
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

    fn name_expr(name: &str) -> ast::ExprName {
        let Expr::Name(name) = py_expr!("{name:id}", name = name) else {
            unreachable!();
        };
        name
    }

    #[test]
    fn stmt_conversion_to_no_await_rejects_await() {
        let stmt = BlockPyStmt::Expr(CoreBlockPyExpr::Await(CoreBlockPyAwait {
            node_index: ast::AtomicNodeIndex::default(),
            range: ruff_text_size::TextRange::default(),
            value: Box::new(CoreBlockPyExpr::Name(name_expr("x"))),
        }));

        assert!(BlockPyStmt::<CoreBlockPyExprWithoutAwait>::try_from(stmt).is_err());
    }

    #[test]
    fn term_conversion_to_no_yield_rejects_nested_yield() {
        let term = BlockPyTerm::Return(Some(CoreBlockPyExprWithoutAwait::Call(CoreBlockPyCall {
            node_index: ast::AtomicNodeIndex::default(),
            range: ruff_text_size::TextRange::default(),
            func: Box::new(CoreBlockPyExprWithoutAwait::Name(name_expr("f"))),
            args: vec![CoreBlockPyCallArg::Positional(
                CoreBlockPyExprWithoutAwait::Yield(CoreBlockPyYield {
                    node_index: ast::AtomicNodeIndex::default(),
                    range: ruff_text_size::TextRange::default(),
                    value: Some(Box::new(CoreBlockPyExprWithoutAwait::Name(name_expr("x")))),
                }),
            )],
            keywords: Vec::new(),
        })));

        assert!(BlockPyTerm::<CoreBlockPyExprWithoutAwaitOrYield>::try_from(term).is_err());
    }
}

impl From<CoreBlockPyExpr> for Expr {
    fn from(value: CoreBlockPyExpr) -> Self {
        match value {
            CoreBlockPyExpr::Literal(literal) => match literal {
                CoreBlockPyLiteral::StringLiteral(node) => Expr::StringLiteral(node),
                CoreBlockPyLiteral::BytesLiteral(node) => Expr::BytesLiteral(node),
                CoreBlockPyLiteral::NumberLiteral(node) => Expr::NumberLiteral(node),
                CoreBlockPyLiteral::BooleanLiteral(node) => Expr::BooleanLiteral(node),
                CoreBlockPyLiteral::NoneLiteral(node) => Expr::NoneLiteral(node),
                CoreBlockPyLiteral::EllipsisLiteral(node) => Expr::EllipsisLiteral(node),
            },
            CoreBlockPyExpr::Call(node) => Expr::Call(ast::ExprCall {
                node_index: node.node_index,
                range: node.range,
                func: Box::new(Expr::from(*node.func)),
                arguments: ast::Arguments {
                    args: node
                        .args
                        .into_iter()
                        .map(|arg| match arg {
                            CoreBlockPyCallArg::Positional(expr) => Expr::from(expr),
                            CoreBlockPyCallArg::Starred(expr) => Expr::Starred(ast::ExprStarred {
                                value: Box::new(Expr::from(expr)),
                                ctx: ast::ExprContext::Load,
                                range: Default::default(),
                                node_index: ast::AtomicNodeIndex::default(),
                            }),
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    keywords: node
                        .keywords
                        .into_iter()
                        .map(|keyword| match keyword {
                            CoreBlockPyKeywordArg::Named { arg, value } => ast::Keyword {
                                arg: Some(arg),
                                value: Expr::from(value),
                                range: Default::default(),
                                node_index: ast::AtomicNodeIndex::default(),
                            },
                            CoreBlockPyKeywordArg::Starred(expr) => ast::Keyword {
                                arg: None,
                                value: Expr::from(expr),
                                range: Default::default(),
                                node_index: ast::AtomicNodeIndex::default(),
                            },
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    range: Default::default(),
                    node_index: ast::AtomicNodeIndex::default(),
                },
            }),
            CoreBlockPyExpr::Await(node) => Expr::Await(ast::ExprAwait {
                node_index: node.node_index,
                range: node.range,
                value: Box::new(Expr::from(*node.value)),
            }),
            CoreBlockPyExpr::Yield(node) => Expr::Yield(ast::ExprYield {
                node_index: node.node_index,
                range: node.range,
                value: node.value.map(|value| Box::new(Expr::from(*value))),
            }),
            CoreBlockPyExpr::YieldFrom(node) => Expr::YieldFrom(ast::ExprYieldFrom {
                node_index: node.node_index,
                range: node.range,
                value: Box::new(Expr::from(*node.value)),
            }),
            CoreBlockPyExpr::Name(node) => Expr::Name(node),
        }
    }
}

impl From<CoreBlockPyExprWithoutAwait> for Expr {
    fn from(value: CoreBlockPyExprWithoutAwait) -> Self {
        match value {
            CoreBlockPyExprWithoutAwait::Literal(literal) => match literal {
                CoreBlockPyLiteral::StringLiteral(node) => Expr::StringLiteral(node),
                CoreBlockPyLiteral::BytesLiteral(node) => Expr::BytesLiteral(node),
                CoreBlockPyLiteral::NumberLiteral(node) => Expr::NumberLiteral(node),
                CoreBlockPyLiteral::BooleanLiteral(node) => Expr::BooleanLiteral(node),
                CoreBlockPyLiteral::NoneLiteral(node) => Expr::NoneLiteral(node),
                CoreBlockPyLiteral::EllipsisLiteral(node) => Expr::EllipsisLiteral(node),
            },
            CoreBlockPyExprWithoutAwait::Call(node) => Expr::Call(ast::ExprCall {
                node_index: node.node_index,
                range: node.range,
                func: Box::new(Expr::from(*node.func)),
                arguments: ast::Arguments {
                    args: node
                        .args
                        .into_iter()
                        .map(|arg| match arg {
                            CoreBlockPyCallArg::Positional(expr) => Expr::from(expr),
                            CoreBlockPyCallArg::Starred(expr) => Expr::Starred(ast::ExprStarred {
                                value: Box::new(Expr::from(expr)),
                                ctx: ast::ExprContext::Load,
                                range: Default::default(),
                                node_index: ast::AtomicNodeIndex::default(),
                            }),
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    keywords: node
                        .keywords
                        .into_iter()
                        .map(|keyword| match keyword {
                            CoreBlockPyKeywordArg::Named { arg, value } => ast::Keyword {
                                arg: Some(arg),
                                value: Expr::from(value),
                                range: Default::default(),
                                node_index: ast::AtomicNodeIndex::default(),
                            },
                            CoreBlockPyKeywordArg::Starred(expr) => ast::Keyword {
                                arg: None,
                                value: Expr::from(expr),
                                range: Default::default(),
                                node_index: ast::AtomicNodeIndex::default(),
                            },
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    range: Default::default(),
                    node_index: ast::AtomicNodeIndex::default(),
                },
            }),
            CoreBlockPyExprWithoutAwait::Yield(node) => Expr::Yield(ast::ExprYield {
                node_index: node.node_index,
                range: node.range,
                value: node.value.map(|value| Box::new(Expr::from(*value))),
            }),
            CoreBlockPyExprWithoutAwait::YieldFrom(node) => Expr::YieldFrom(ast::ExprYieldFrom {
                node_index: node.node_index,
                range: node.range,
                value: Box::new(Expr::from(*node.value)),
            }),
            CoreBlockPyExprWithoutAwait::Name(node) => Expr::Name(node),
        }
    }
}

impl From<CoreBlockPyExprWithoutAwaitOrYield> for Expr {
    fn from(value: CoreBlockPyExprWithoutAwaitOrYield) -> Self {
        match value {
            CoreBlockPyExprWithoutAwaitOrYield::Literal(literal) => match literal {
                CoreBlockPyLiteral::StringLiteral(node) => Expr::StringLiteral(node),
                CoreBlockPyLiteral::BytesLiteral(node) => Expr::BytesLiteral(node),
                CoreBlockPyLiteral::NumberLiteral(node) => Expr::NumberLiteral(node),
                CoreBlockPyLiteral::BooleanLiteral(node) => Expr::BooleanLiteral(node),
                CoreBlockPyLiteral::NoneLiteral(node) => Expr::NoneLiteral(node),
                CoreBlockPyLiteral::EllipsisLiteral(node) => Expr::EllipsisLiteral(node),
            },
            CoreBlockPyExprWithoutAwaitOrYield::Call(node) => Expr::Call(ast::ExprCall {
                node_index: node.node_index,
                range: node.range,
                func: Box::new(Expr::from(*node.func)),
                arguments: ast::Arguments {
                    args: node
                        .args
                        .into_iter()
                        .map(|arg| match arg {
                            CoreBlockPyCallArg::Positional(expr) => Expr::from(expr),
                            CoreBlockPyCallArg::Starred(expr) => Expr::Starred(ast::ExprStarred {
                                value: Box::new(Expr::from(expr)),
                                ctx: ast::ExprContext::Load,
                                range: Default::default(),
                                node_index: ast::AtomicNodeIndex::default(),
                            }),
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    keywords: node
                        .keywords
                        .into_iter()
                        .map(|keyword| match keyword {
                            CoreBlockPyKeywordArg::Named { arg, value } => ast::Keyword {
                                arg: Some(arg),
                                value: Expr::from(value),
                                range: Default::default(),
                                node_index: ast::AtomicNodeIndex::default(),
                            },
                            CoreBlockPyKeywordArg::Starred(expr) => ast::Keyword {
                                arg: None,
                                value: Expr::from(expr),
                                range: Default::default(),
                                node_index: ast::AtomicNodeIndex::default(),
                            },
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                    range: Default::default(),
                    node_index: ast::AtomicNodeIndex::default(),
                },
            }),
            CoreBlockPyExprWithoutAwaitOrYield::Name(node) => Expr::Name(node),
        }
    }
}

impl TryFrom<CoreBlockPyExpr> for CoreBlockPyExprWithoutAwait {
    type Error = CoreBlockPyExpr;

    fn try_from(value: CoreBlockPyExpr) -> Result<Self, Self::Error> {
        match value {
            CoreBlockPyExpr::Name(node) => Ok(Self::Name(node)),
            CoreBlockPyExpr::Literal(literal) => Ok(Self::Literal(literal)),
            CoreBlockPyExpr::Call(call) => Ok(Self::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(Self::try_from(*call.func)?),
                args: call
                    .args
                    .into_iter()
                    .map(|arg| match arg {
                        CoreBlockPyCallArg::Positional(expr) => {
                            Self::try_from(expr).map(CoreBlockPyCallArg::Positional)
                        }
                        CoreBlockPyCallArg::Starred(expr) => {
                            Self::try_from(expr).map(CoreBlockPyCallArg::Starred)
                        }
                    })
                    .collect::<Result<_, _>>()?,
                keywords: call
                    .keywords
                    .into_iter()
                    .map(|keyword| match keyword {
                        CoreBlockPyKeywordArg::Named { arg, value } => Self::try_from(value)
                            .map(|value| CoreBlockPyKeywordArg::Named { arg, value }),
                        CoreBlockPyKeywordArg::Starred(value) => {
                            Self::try_from(value).map(CoreBlockPyKeywordArg::Starred)
                        }
                    })
                    .collect::<Result<_, _>>()?,
            })),
            CoreBlockPyExpr::Yield(yield_expr) => Ok(Self::Yield(CoreBlockPyYield {
                node_index: yield_expr.node_index,
                range: yield_expr.range,
                value: yield_expr
                    .value
                    .map(|value| Self::try_from(*value).map(Box::new))
                    .transpose()?,
            })),
            CoreBlockPyExpr::YieldFrom(yield_from_expr) => {
                Ok(Self::YieldFrom(CoreBlockPyYieldFrom {
                    node_index: yield_from_expr.node_index,
                    range: yield_from_expr.range,
                    value: Box::new(Self::try_from(*yield_from_expr.value)?),
                }))
            }
            CoreBlockPyExpr::Await(_) => Err(value),
        }
    }
}

impl TryFrom<BlockPyStmt<CoreBlockPyExpr>> for BlockPyStmt<CoreBlockPyExprWithoutAwait> {
    type Error = CoreBlockPyExpr;

    fn try_from(value: BlockPyStmt<CoreBlockPyExpr>) -> Result<Self, Self::Error> {
        match value {
            BlockPyStmt::Assign(assign) => Ok(BlockPyStmt::Assign(BlockPyAssign {
                target: assign.target,
                value: assign.value.try_into()?,
            })),
            BlockPyStmt::Expr(expr) => Ok(BlockPyStmt::Expr(expr.try_into()?)),
            BlockPyStmt::Delete(delete) => Ok(BlockPyStmt::Delete(delete)),
            BlockPyStmt::If(if_stmt) => Ok(BlockPyStmt::If(BlockPyIf {
                test: if_stmt.test.try_into()?,
                body: if_stmt.body.try_into()?,
                orelse: if_stmt.orelse.try_into()?,
            })),
        }
    }
}

impl TryFrom<BlockPyTerm<CoreBlockPyExpr>> for BlockPyTerm<CoreBlockPyExprWithoutAwait> {
    type Error = CoreBlockPyExpr;

    fn try_from(value: BlockPyTerm<CoreBlockPyExpr>) -> Result<Self, Self::Error> {
        match value {
            BlockPyTerm::Jump(target) => Ok(BlockPyTerm::Jump(target)),
            BlockPyTerm::IfTerm(if_term) => Ok(BlockPyTerm::IfTerm(BlockPyIfTerm {
                test: if_term.test.try_into()?,
                then_label: if_term.then_label,
                else_label: if_term.else_label,
            })),
            BlockPyTerm::BranchTable(branch) => Ok(BlockPyTerm::BranchTable(BlockPyBranchTable {
                index: branch.index.try_into()?,
                targets: branch.targets,
                default_label: branch.default_label,
            })),
            BlockPyTerm::Raise(raise_stmt) => Ok(BlockPyTerm::Raise(BlockPyRaise {
                exc: raise_stmt.exc.map(TryInto::try_into).transpose()?,
            })),
            BlockPyTerm::TryJump(try_jump) => Ok(BlockPyTerm::TryJump(try_jump)),
            BlockPyTerm::Return(value) => Ok(BlockPyTerm::Return(
                value.map(TryInto::try_into).transpose()?,
            )),
        }
    }
}

impl TryFrom<BlockPyCfgFragment<BlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>>>
    for BlockPyCfgFragment<
        BlockPyStmt<CoreBlockPyExprWithoutAwait>,
        BlockPyTerm<CoreBlockPyExprWithoutAwait>,
    >
{
    type Error = CoreBlockPyExpr;

    fn try_from(
        value: BlockPyCfgFragment<BlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>>,
    ) -> Result<Self, Self::Error> {
        Ok(BlockPyCfgFragment::with_term(
            value
                .body
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
            value.term.map(TryInto::try_into).transpose()?,
        ))
    }
}

impl TryFrom<BlockPyBlock<CoreBlockPyExpr>> for BlockPyBlock<CoreBlockPyExprWithoutAwait> {
    type Error = CoreBlockPyExpr;

    fn try_from(value: BlockPyBlock<CoreBlockPyExpr>) -> Result<Self, Self::Error> {
        Ok(BlockPyBlock {
            label: value.label,
            body: value
                .body
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
            term: value.term.try_into()?,
            meta: value.meta,
        })
    }
}

impl TryFrom<BlockPyCallableDef<CoreBlockPyExpr>>
    for BlockPyCallableDef<CoreBlockPyExprWithoutAwait>
{
    type Error = CoreBlockPyExpr;

    fn try_from(value: BlockPyCallableDef<CoreBlockPyExpr>) -> Result<Self, Self::Error> {
        let BlockPyCallableDef {
            function_id,
            names,
            kind,
            params,
            param_defaults,
            blocks,
            doc,
            closure_layout,
            facts,
            try_regions,
        } = value;
        Ok(BlockPyCallableDef {
            function_id,
            names,
            kind,
            params,
            param_defaults: param_defaults
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
            blocks: blocks
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
            doc,
            closure_layout,
            facts,
            try_regions,
        })
    }
}

impl From<CoreBlockPyExprWithoutAwait> for CoreBlockPyExpr {
    fn from(value: CoreBlockPyExprWithoutAwait) -> Self {
        match value {
            CoreBlockPyExprWithoutAwait::Name(node) => Self::Name(node),
            CoreBlockPyExprWithoutAwait::Literal(literal) => Self::Literal(literal),
            CoreBlockPyExprWithoutAwait::Call(call) => Self::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(Self::from(*call.func)),
                args: call
                    .args
                    .into_iter()
                    .map(|arg| match arg {
                        CoreBlockPyCallArg::Positional(expr) => {
                            CoreBlockPyCallArg::Positional(Self::from(expr))
                        }
                        CoreBlockPyCallArg::Starred(expr) => {
                            CoreBlockPyCallArg::Starred(Self::from(expr))
                        }
                    })
                    .collect(),
                keywords: call
                    .keywords
                    .into_iter()
                    .map(|keyword| match keyword {
                        CoreBlockPyKeywordArg::Named { arg, value } => {
                            CoreBlockPyKeywordArg::Named {
                                arg,
                                value: Self::from(value),
                            }
                        }
                        CoreBlockPyKeywordArg::Starred(value) => {
                            CoreBlockPyKeywordArg::Starred(Self::from(value))
                        }
                    })
                    .collect(),
            }),
            CoreBlockPyExprWithoutAwait::Yield(yield_expr) => Self::Yield(CoreBlockPyYield {
                node_index: yield_expr.node_index,
                range: yield_expr.range,
                value: yield_expr.value.map(|value| Box::new(Self::from(*value))),
            }),
            CoreBlockPyExprWithoutAwait::YieldFrom(yield_from_expr) => {
                Self::YieldFrom(CoreBlockPyYieldFrom {
                    node_index: yield_from_expr.node_index,
                    range: yield_from_expr.range,
                    value: Box::new(Self::from(*yield_from_expr.value)),
                })
            }
        }
    }
}

impl TryFrom<CoreBlockPyExprWithoutAwait> for CoreBlockPyExprWithoutAwaitOrYield {
    type Error = CoreBlockPyExprWithoutAwait;

    fn try_from(value: CoreBlockPyExprWithoutAwait) -> Result<Self, Self::Error> {
        match value {
            CoreBlockPyExprWithoutAwait::Name(node) => Ok(Self::Name(node)),
            CoreBlockPyExprWithoutAwait::Literal(literal) => Ok(Self::Literal(literal)),
            CoreBlockPyExprWithoutAwait::Call(call) => Ok(Self::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(Self::try_from(*call.func)?),
                args: call
                    .args
                    .into_iter()
                    .map(|arg| match arg {
                        CoreBlockPyCallArg::Positional(expr) => {
                            Self::try_from(expr).map(CoreBlockPyCallArg::Positional)
                        }
                        CoreBlockPyCallArg::Starred(expr) => {
                            Self::try_from(expr).map(CoreBlockPyCallArg::Starred)
                        }
                    })
                    .collect::<Result<_, _>>()?,
                keywords: call
                    .keywords
                    .into_iter()
                    .map(|keyword| match keyword {
                        CoreBlockPyKeywordArg::Named { arg, value } => Self::try_from(value)
                            .map(|value| CoreBlockPyKeywordArg::Named { arg, value }),
                        CoreBlockPyKeywordArg::Starred(value) => {
                            Self::try_from(value).map(CoreBlockPyKeywordArg::Starred)
                        }
                    })
                    .collect::<Result<_, _>>()?,
            })),
            CoreBlockPyExprWithoutAwait::Yield(_) | CoreBlockPyExprWithoutAwait::YieldFrom(_) => {
                Err(value)
            }
        }
    }
}

impl TryFrom<BlockPyStmt<CoreBlockPyExprWithoutAwait>>
    for BlockPyStmt<CoreBlockPyExprWithoutAwaitOrYield>
{
    type Error = CoreBlockPyExprWithoutAwait;

    fn try_from(value: BlockPyStmt<CoreBlockPyExprWithoutAwait>) -> Result<Self, Self::Error> {
        match value {
            BlockPyStmt::Assign(assign) => Ok(BlockPyStmt::Assign(BlockPyAssign {
                target: assign.target,
                value: assign.value.try_into()?,
            })),
            BlockPyStmt::Expr(expr) => Ok(BlockPyStmt::Expr(expr.try_into()?)),
            BlockPyStmt::Delete(delete) => Ok(BlockPyStmt::Delete(delete)),
            BlockPyStmt::If(if_stmt) => Ok(BlockPyStmt::If(BlockPyIf {
                test: if_stmt.test.try_into()?,
                body: if_stmt.body.try_into()?,
                orelse: if_stmt.orelse.try_into()?,
            })),
        }
    }
}

impl TryFrom<BlockPyTerm<CoreBlockPyExprWithoutAwait>>
    for BlockPyTerm<CoreBlockPyExprWithoutAwaitOrYield>
{
    type Error = CoreBlockPyExprWithoutAwait;

    fn try_from(value: BlockPyTerm<CoreBlockPyExprWithoutAwait>) -> Result<Self, Self::Error> {
        match value {
            BlockPyTerm::Jump(target) => Ok(BlockPyTerm::Jump(target)),
            BlockPyTerm::IfTerm(if_term) => Ok(BlockPyTerm::IfTerm(BlockPyIfTerm {
                test: if_term.test.try_into()?,
                then_label: if_term.then_label,
                else_label: if_term.else_label,
            })),
            BlockPyTerm::BranchTable(branch) => Ok(BlockPyTerm::BranchTable(BlockPyBranchTable {
                index: branch.index.try_into()?,
                targets: branch.targets,
                default_label: branch.default_label,
            })),
            BlockPyTerm::Raise(raise_stmt) => Ok(BlockPyTerm::Raise(BlockPyRaise {
                exc: raise_stmt.exc.map(TryInto::try_into).transpose()?,
            })),
            BlockPyTerm::TryJump(try_jump) => Ok(BlockPyTerm::TryJump(try_jump)),
            BlockPyTerm::Return(value) => Ok(BlockPyTerm::Return(
                value.map(TryInto::try_into).transpose()?,
            )),
        }
    }
}

impl
    TryFrom<
        BlockPyCfgFragment<
            BlockPyStmt<CoreBlockPyExprWithoutAwait>,
            BlockPyTerm<CoreBlockPyExprWithoutAwait>,
        >,
    >
    for BlockPyCfgFragment<
        BlockPyStmt<CoreBlockPyExprWithoutAwaitOrYield>,
        BlockPyTerm<CoreBlockPyExprWithoutAwaitOrYield>,
    >
{
    type Error = CoreBlockPyExprWithoutAwait;

    fn try_from(
        value: BlockPyCfgFragment<
            BlockPyStmt<CoreBlockPyExprWithoutAwait>,
            BlockPyTerm<CoreBlockPyExprWithoutAwait>,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(BlockPyCfgFragment::with_term(
            value
                .body
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
            value.term.map(TryInto::try_into).transpose()?,
        ))
    }
}

impl TryFrom<BlockPyBlock<CoreBlockPyExprWithoutAwait>>
    for BlockPyBlock<CoreBlockPyExprWithoutAwaitOrYield>
{
    type Error = CoreBlockPyExprWithoutAwait;

    fn try_from(value: BlockPyBlock<CoreBlockPyExprWithoutAwait>) -> Result<Self, Self::Error> {
        Ok(BlockPyBlock {
            label: value.label,
            body: value
                .body
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
            term: value.term.try_into()?,
            meta: value.meta,
        })
    }
}

impl TryFrom<BlockPyCallableDef<CoreBlockPyExprWithoutAwait>>
    for BlockPyCallableDef<CoreBlockPyExprWithoutAwaitOrYield>
{
    type Error = CoreBlockPyExprWithoutAwait;

    fn try_from(
        value: BlockPyCallableDef<CoreBlockPyExprWithoutAwait>,
    ) -> Result<Self, Self::Error> {
        let BlockPyCallableDef {
            function_id,
            names,
            kind,
            params,
            param_defaults,
            blocks,
            doc,
            closure_layout,
            facts,
            try_regions,
        } = value;
        Ok(BlockPyCallableDef {
            function_id,
            names,
            kind,
            params,
            param_defaults: param_defaults
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
            blocks: blocks
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
            doc,
            closure_layout,
            facts,
            try_regions,
        })
    }
}

impl From<CoreBlockPyExprWithoutAwaitOrYield> for CoreBlockPyExprWithoutAwait {
    fn from(value: CoreBlockPyExprWithoutAwaitOrYield) -> Self {
        match value {
            CoreBlockPyExprWithoutAwaitOrYield::Name(node) => Self::Name(node),
            CoreBlockPyExprWithoutAwaitOrYield::Literal(literal) => Self::Literal(literal),
            CoreBlockPyExprWithoutAwaitOrYield::Call(call) => Self::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(Self::from(*call.func)),
                args: call
                    .args
                    .into_iter()
                    .map(|arg| match arg {
                        CoreBlockPyCallArg::Positional(expr) => {
                            CoreBlockPyCallArg::Positional(Self::from(expr))
                        }
                        CoreBlockPyCallArg::Starred(expr) => {
                            CoreBlockPyCallArg::Starred(Self::from(expr))
                        }
                    })
                    .collect(),
                keywords: call
                    .keywords
                    .into_iter()
                    .map(|keyword| match keyword {
                        CoreBlockPyKeywordArg::Named { arg, value } => {
                            CoreBlockPyKeywordArg::Named {
                                arg,
                                value: Self::from(value),
                            }
                        }
                        CoreBlockPyKeywordArg::Starred(value) => {
                            CoreBlockPyKeywordArg::Starred(Self::from(value))
                        }
                    })
                    .collect(),
            }),
        }
    }
}

impl From<CoreBlockPyExprWithoutAwaitOrYield> for CoreBlockPyExpr {
    fn from(value: CoreBlockPyExprWithoutAwaitOrYield) -> Self {
        Self::from(CoreBlockPyExprWithoutAwait::from(value))
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

impl Borrow<str> for BlockPyLabel {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for BlockPyLabel {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for BlockPyLabel {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl fmt::Display for BlockPyLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq<&str> for BlockPyLabel {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}
