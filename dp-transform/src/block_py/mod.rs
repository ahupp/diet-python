use self::param_specs::ParamSpec;
use crate::block_py::dataflow::{
    compute_block_params_blockpy, extend_state_order_with_declared_block_params,
    merge_declared_block_params,
};
use crate::block_py::state::collect_state_vars;
use crate::passes::ruff_to_blockpy;
use crate::passes::{
    BbBlockPyPass, CoreBlockPyPass, CoreBlockPyPassWithoutAwait,
    CoreBlockPyPassWithoutAwaitOrYield, PreparedBbBlockPyPass, RuffBlockPyPass,
};
use crate::py_expr;
use ruff_python_ast::str::Quote;
pub use ruff_python_ast::Expr;
use ruff_python_ast::{
    self as ast, BytesLiteral, BytesLiteralFlags, ExprName, StringLiteral, StringLiteralFlags,
    StringLiteralValue,
};
use std::borrow::Borrow;
use std::collections::HashSet;
use std::fmt;
use std::ops::Deref;

pub(crate) mod cfg;
pub(crate) mod dataflow;
pub(crate) mod exception;
pub(crate) mod param_specs;
pub mod pretty;
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ResumeAbiParam {
    SelfValue,
    SendValue,
    ResumeExc,
    TransportSent,
}

impl ResumeAbiParam {
    pub(crate) fn name(self) -> &'static str {
        match self {
            ResumeAbiParam::SelfValue => "_dp_self",
            ResumeAbiParam::SendValue => "_dp_send_value",
            ResumeAbiParam::ResumeExc => "_dp_resume_exc",
            ResumeAbiParam::TransportSent => "_dp_transport_sent",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<Self> {
        match name {
            "_dp_self" => Some(ResumeAbiParam::SelfValue),
            "_dp_send_value" => Some(ResumeAbiParam::SendValue),
            "_dp_resume_exc" => Some(ResumeAbiParam::ResumeExc),
            "_dp_transport_sent" => Some(ResumeAbiParam::TransportSent),
            _ => None,
        }
    }
}

const GENERATOR_RESUME_ABI_PARAMS: [ResumeAbiParam; 3] = [
    ResumeAbiParam::SelfValue,
    ResumeAbiParam::SendValue,
    ResumeAbiParam::ResumeExc,
];

const ASYNC_GENERATOR_RESUME_ABI_PARAMS: [ResumeAbiParam; 4] = [
    ResumeAbiParam::SelfValue,
    ResumeAbiParam::SendValue,
    ResumeAbiParam::ResumeExc,
    ResumeAbiParam::TransportSent,
];

pub(crate) fn resume_abi_params(kind: BlockPyFunctionKind) -> &'static [ResumeAbiParam] {
    match kind {
        BlockPyFunctionKind::Function => &[],
        BlockPyFunctionKind::Coroutine | BlockPyFunctionKind::Generator => {
            &GENERATOR_RESUME_ABI_PARAMS
        }
        BlockPyFunctionKind::AsyncGenerator => &ASYNC_GENERATOR_RESUME_ABI_PARAMS,
    }
}

pub(crate) fn is_resume_abi_param_name(name: &str) -> bool {
    ResumeAbiParam::from_name(name).is_some()
}

#[derive(Debug, Clone)]
pub struct CfgBlock<S, T> {
    pub label: BlockPyLabel,
    pub body: Vec<S>,
    pub term: T,
    pub params: Vec<BlockParam>,
    pub exc_edge: Option<BlockPyEdge>,
}

impl<S, T> CfgBlock<S, T> {
    pub fn label_str(&self) -> &str {
        self.label.as_str()
    }

    pub fn ensure_param(&mut self, name: impl Into<String>, role: BlockParamRole) {
        let name = name.into();
        if self.params.iter().any(|param| param.name == name) {
            return;
        }
        self.params.push(BlockParam { name, role });
    }

    pub fn set_exception_param(&mut self, name: impl Into<String>) {
        let name = name.into();
        for param in &mut self.params {
            if param.role == BlockParamRole::Exception && param.name != name {
                param.role = BlockParamRole::Local;
            }
        }
        if let Some(param) = self.params.iter_mut().find(|param| param.name == name) {
            param.role = BlockParamRole::Exception;
            return;
        }
        self.params.push(BlockParam {
            name,
            role: BlockParamRole::Exception,
        });
    }

    pub fn exception_param(&self) -> Option<&str> {
        self.params
            .iter()
            .find(|param| param.role == BlockParamRole::Exception)
            .map(|param| param.name.as_str())
    }

    pub fn param_names(&self) -> impl Iterator<Item = &str> {
        self.params.iter().map(|param| param.name.as_str())
    }

    pub fn param_name_vec(&self) -> Vec<String> {
        self.param_names().map(ToString::to_string).collect()
    }

    pub fn bb_params(&self) -> impl Iterator<Item = &BlockParam> {
        [
            BlockParamRole::Exception,
            BlockParamRole::Local,
            BlockParamRole::AbruptKind,
            BlockParamRole::AbruptPayload,
        ]
        .into_iter()
        .flat_map(|role| self.params.iter().filter(move |param| param.role == role))
    }

    pub fn bb_param_names(&self) -> impl Iterator<Item = &str> {
        self.bb_params().map(|param| param.name.as_str())
    }
}

#[derive(Debug, Clone, Default)]
pub struct BlockPyModule<P: BlockPyPass> {
    pub callable_defs: Vec<BlockPyFunction<P>>,
}

impl<P: BlockPyPass> BlockPyModule<P> {
    pub fn map_callable_defs<Q: BlockPyPass>(
        self,
        mut f: impl FnMut(BlockPyFunction<P>) -> BlockPyFunction<Q>,
    ) -> BlockPyModule<Q> {
        BlockPyModule {
            callable_defs: self.callable_defs.into_iter().map(&mut f).collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum CoreBlockPyExpr {
    Name(ast::ExprName),
    Literal(CoreBlockPyLiteral),
    Call(CoreBlockPyCall<CoreBlockPyExpr>),
    Await(CoreBlockPyAwait<CoreBlockPyExpr>),
    Yield(CoreBlockPyYield<CoreBlockPyExpr>),
    YieldFrom(CoreBlockPyYieldFrom<CoreBlockPyExpr>),
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
    StringLiteral(CoreStringLiteral),
    BytesLiteral(CoreBytesLiteral),
    NumberLiteral(CoreNumberLiteral),
}

#[derive(Debug, Clone)]
pub struct CoreStringLiteral {
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct CoreBytesLiteral {
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct CoreNumberLiteral {
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub value: CoreNumberLiteralValue,
}

#[derive(Debug, Clone)]
pub enum CoreNumberLiteralValue {
    Int(ast::Int),
    Float(f64),
}

#[derive(Debug, Clone)]
pub struct CoreBlockPyCall<E> {
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
pub struct CoreBlockPyAwait<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub value: Box<E>,
}

#[derive(Debug, Clone)]
pub struct CoreBlockPyYield<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub value: Option<Box<E>>,
}

#[derive(Debug, Clone)]
pub struct CoreBlockPyYieldFrom<E> {
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub value: Box<E>,
}

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
pub struct BlockPyFunction<P: BlockPyPass> {
    pub function_id: FunctionId,
    pub names: FunctionName,
    pub kind: BlockPyFunctionKind,
    pub params: ParamSpec,
    pub blocks: Vec<CfgBlock<P::Stmt, BlockPyTerm<P::Expr>>>,
    pub doc: Option<String>,
    pub closure_layout: Option<ClosureLayout>,
    pub facts: BlockPyCallableFacts,
    pub try_regions: Vec<TryRegionPlan>,
}

impl<P: BlockPyPass> BlockPyFunction<P> {
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

    pub fn entry_block(&self) -> &PassBlock<P> {
        self.blocks
            .first()
            .expect("BlockPyFunction should have at least one block")
    }

    pub fn map_blocks<Q: BlockPyPass>(
        self,
        mut f: impl FnMut(PassBlock<P>) -> PassBlock<Q>,
    ) -> BlockPyFunction<Q>
    where
        Q: BlockPyPass<Expr = P::Expr>,
    {
        BlockPyFunction {
            function_id: self.function_id,
            names: self.names,
            kind: self.kind,
            params: self.params,
            blocks: self.blocks.into_iter().map(&mut f).collect(),
            doc: self.doc,
            closure_layout: self.closure_layout,
            facts: self.facts,
            try_regions: self.try_regions,
        }
    }
}

pub fn lowered_entry_liveins<S, E>(
    params: &ParamSpec,
    blocks: &[CfgBlock<S, BlockPyTerm<E>>],
) -> Vec<String>
where
    S: IntoBlockPyStmt<E>,
    E: Clone + Into<Expr> + fmt::Debug,
{
    if blocks.is_empty() {
        return Vec::new();
    }
    let lowered_blocks = blocks
        .iter()
        .map(|block| CfgBlock {
            label: block.label.clone(),
            body: block
                .body
                .iter()
                .map(|stmt| stmt.clone().into_stmt())
                .collect(),
            term: block.term.clone(),
            params: block.params.clone(),
            exc_edge: block.exc_edge.clone(),
        })
        .collect::<Vec<_>>();
    let param_names = params.names();
    let mut state_vars = collect_state_vars(&param_names, &lowered_blocks);
    extend_state_order_with_declared_block_params(&lowered_blocks, &mut state_vars);
    let mut block_params = compute_block_params_blockpy(
        &lowered_blocks,
        &state_vars,
        &ruff_to_blockpy::lowered_exception_edges(&lowered_blocks)
            .into_iter()
            .filter_map(|(source, target)| target.map(|target| (source, vec![target])))
            .collect(),
    );
    merge_declared_block_params(&lowered_blocks, &mut block_params);
    block_params
        .get(lowered_blocks[0].label.as_str())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|name| !is_resume_abi_param_name(name))
        .collect()
}

macro_rules! impl_non_bb_entry_liveins {
    ($($pass:ty),* $(,)?) => {
        $(
            impl BlockPyFunction<$pass> {
                pub fn entry_liveins(&self) -> Vec<String> {
                    lowered_entry_liveins(&self.params, &self.blocks)
                }
            }
        )*
    };
}

impl_non_bb_entry_liveins!(
    RuffBlockPyPass,
    CoreBlockPyPass,
    CoreBlockPyPassWithoutAwait,
    CoreBlockPyPassWithoutAwaitOrYield,
);

impl BlockPyFunction<BbBlockPyPass> {
    pub fn entry_liveins(&self) -> Vec<String> {
        if self.blocks.is_empty() {
            return Vec::new();
        }
        self.entry_block()
            .param_names()
            .filter(|name| !is_resume_abi_param_name(name))
            .map(ToString::to_string)
            .collect()
    }
}

impl BlockPyFunction<PreparedBbBlockPyPass> {
    pub fn entry_liveins(&self) -> Vec<String> {
        if self.blocks.is_empty() {
            return Vec::new();
        }
        self.entry_block()
            .param_names()
            .filter(|name| !is_resume_abi_param_name(name))
            .map(ToString::to_string)
            .collect()
    }
}

pub trait BlockPyNormalizedStmt {
    fn assert_blockpy_normalized(&self);
}

pub trait IntoBlockPyStmt<E>: Clone + fmt::Debug {
    fn into_stmt(self) -> BlockPyStmt<E>;
}

pub trait BlockPyPass: Clone + fmt::Debug {
    type Expr: Clone + fmt::Debug + Into<Expr>;
    type Stmt: BlockPyNormalizedStmt + IntoBlockPyStmt<Self::Expr>;
}

pub type PassExpr<P> = <P as BlockPyPass>::Expr;
pub type PassBlock<P> = CfgBlock<<P as BlockPyPass>::Stmt, BlockPyTerm<PassExpr<P>>>;
pub type BbBlock = PassBlock<BbBlockPyPass>;
pub type PreparedBbBlock = PassBlock<PreparedBbBlockPyPass>;

pub type BlockPyCfgBlock<S, T> = CfgBlock<S, T>;
pub type BlockPyBlock<E = Expr> = BlockPyCfgBlock<BlockPyStmt<E>, BlockPyTerm<E>>;
pub type BlockPyStructuredIf<E = Expr> = BlockPyIf<E, BlockPyStmt<E>, BlockPyTerm<E>>;

pub trait BlockPyJumpTerm<L> {
    fn jump_term(target: L) -> Self;
}

pub trait BlockPyFallthroughTerm<L>: BlockPyJumpTerm<L> {
    fn implicit_function_return() -> Self;
}

pub(crate) trait ImplicitNoneExpr {
    fn implicit_none_expr() -> Self;
    fn is_implicit_none_expr(expr: &Self) -> bool;
}

fn implicit_none_name() -> ast::ExprName {
    let Expr::Name(name) = py_expr!("__dp_NONE") else {
        unreachable!();
    };
    name
}

pub fn assert_blockpy_block_normalized<S: BlockPyNormalizedStmt, T>(block: &CfgBlock<S, T>) {
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
    params: Vec<BlockParam>,
    exc_edge: Option<BlockPyEdge>,
    fragment: BlockPyCfgFragmentBuilder<S, T>,
}

pub type BlockPyBlockBuilder<E = Expr> = BlockPyCfgBlockBuilder<BlockPyStmt<E>, BlockPyTerm<E>>;

impl<S: BlockPyNormalizedStmt, T: BlockPyFallthroughTerm<BlockPyLabel>>
    BlockPyCfgBlockBuilder<S, T>
{
    pub fn new(label: BlockPyLabel) -> Self {
        Self {
            label,
            params: Vec::new(),
            exc_edge: None,
            fragment: BlockPyCfgFragmentBuilder::new(),
        }
    }

    pub fn with_exc_param(mut self, exc_param: Option<String>) -> Self {
        if let Some(exc_param) = exc_param {
            if self.params.iter().any(|param| param.name == exc_param) {
                for param in &mut self.params {
                    if param.role == BlockParamRole::Exception && param.name != exc_param {
                        param.role = BlockParamRole::Local;
                    }
                    if param.name == exc_param {
                        param.role = BlockParamRole::Exception;
                    }
                }
            } else {
                for param in &mut self.params {
                    if param.role == BlockParamRole::Exception {
                        param.role = BlockParamRole::Local;
                    }
                }
                self.params.push(BlockParam {
                    name: exc_param,
                    role: BlockParamRole::Exception,
                });
            }
        }
        self
    }

    pub fn with_params(mut self, params: Vec<BlockParam>) -> Self {
        for param in params {
            if !self
                .params
                .iter()
                .any(|existing| existing.name == param.name)
            {
                self.params.push(param);
            }
        }
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
            params: self.params,
            exc_edge: self.exc_edge,
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
pub enum BbStmt {
    Assign(BlockPyAssign<CoreBlockPyExprWithoutAwaitOrYield>),
    Expr(CoreBlockPyExprWithoutAwaitOrYield),
    Delete(BlockPyDelete),
}

impl From<BlockPyAssign<CoreBlockPyExprWithoutAwaitOrYield>> for BbStmt {
    fn from(value: BlockPyAssign<CoreBlockPyExprWithoutAwaitOrYield>) -> Self {
        Self::Assign(value)
    }
}

impl From<CoreBlockPyExprWithoutAwaitOrYield> for BbStmt {
    fn from(value: CoreBlockPyExprWithoutAwaitOrYield) -> Self {
        Self::Expr(value)
    }
}

impl From<BlockPyDelete> for BbStmt {
    fn from(value: BlockPyDelete) -> Self {
        Self::Delete(value)
    }
}

impl From<BlockPyStmt<CoreBlockPyExprWithoutAwaitOrYield>> for BbStmt {
    fn from(value: BlockPyStmt<CoreBlockPyExprWithoutAwaitOrYield>) -> Self {
        match value {
            BlockPyStmt::Assign(assign) => Self::Assign(assign),
            BlockPyStmt::Expr(expr) => Self::Expr(expr),
            BlockPyStmt::Delete(delete) => Self::Delete(delete),
            BlockPyStmt::If(_) => panic!("structured BlockPy If reached BbStmt conversion"),
        }
    }
}

impl IntoBlockPyStmt<CoreBlockPyExprWithoutAwaitOrYield> for BbStmt {
    fn into_stmt(self) -> BlockPyStmt<CoreBlockPyExprWithoutAwaitOrYield> {
        match self {
            BbStmt::Assign(assign) => BlockPyStmt::Assign(assign),
            BbStmt::Expr(expr) => BlockPyStmt::Expr(expr),
            BbStmt::Delete(delete) => BlockPyStmt::Delete(delete),
        }
    }
}

impl BlockPyNormalizedStmt for BbStmt {
    fn assert_blockpy_normalized(&self) {}
}

impl<E: Clone + fmt::Debug> IntoBlockPyStmt<E> for BlockPyStmt<E> {
    fn into_stmt(self) -> BlockPyStmt<E> {
        self
    }
}

pub type BbTerm = BlockPyTerm<CoreBlockPyExprWithoutAwaitOrYield>;

#[derive(Debug, Clone)]
pub enum BlockPyTerm<E = Expr> {
    Jump(BlockPyEdge),
    IfTerm(BlockPyIfTerm<E>),
    BranchTable(BlockPyBranchTable<E>),
    Raise(BlockPyRaise<E>),
    Return(E),
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
pub struct BlockPyEdge {
    pub target: BlockPyLabel,
    pub args: Vec<BlockArg>,
}

impl BlockPyEdge {
    pub fn new(target: BlockPyLabel) -> Self {
        Self {
            target,
            args: Vec::new(),
        }
    }

    pub fn with_args(target: BlockPyLabel, args: Vec<BlockArg>) -> Self {
        Self { target, args }
    }

    pub fn as_str(&self) -> &str {
        self.target.as_str()
    }
}

impl From<BlockPyLabel> for BlockPyEdge {
    fn from(value: BlockPyLabel) -> Self {
        Self::new(value)
    }
}

impl From<&str> for BlockPyEdge {
    fn from(value: &str) -> Self {
        Self::new(BlockPyLabel::from(value))
    }
}

impl From<String> for BlockPyEdge {
    fn from(value: String) -> Self {
        Self::new(BlockPyLabel::from(value))
    }
}

#[derive(Debug, Clone)]
pub enum BlockArg {
    Name(String),
    None,
    CurrentException,
    AbruptKind(AbruptKind),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum AbruptKind {
    Fallthrough,
    Return,
    Exception,
    Break,
    Continue,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockParamRole {
    Local,
    Exception,
    AbruptKind,
    AbruptPayload,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BlockParam {
    pub name: String,
    pub role: BlockParamRole,
}

pub trait BlockPyModuleVisitor<P>
where
    P: BlockPyPass,
{
    fn visit_module(&mut self, module: &BlockPyModule<P>) {
        walk_module(self, module);
    }

    fn visit_fn(&mut self, func: &BlockPyFunction<P>) {
        walk_fn(self, func);
    }

    fn visit_block(&mut self, block: &PassBlock<P>) {
        walk_block(self, block);
    }

    fn visit_fragment(
        &mut self,
        fragment: &BlockPyCfgFragment<BlockPyStmt<PassExpr<P>>, BlockPyTerm<PassExpr<P>>>,
    ) {
        walk_fragment(self, fragment);
    }

    fn visit_stmt(&mut self, stmt: &BlockPyStmt<PassExpr<P>>) {
        walk_stmt(self, stmt);
    }

    fn visit_term(&mut self, term: &BlockPyTerm<PassExpr<P>>) {
        walk_term(self, term);
    }

    fn visit_label(&mut self, label: &BlockPyLabel) {
        walk_label::<Self, P>(self, label);
    }

    fn visit_expr(&mut self, _expr: &PassExpr<P>) {}
}

pub fn walk_module<V, P>(visitor: &mut V, module: &BlockPyModule<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
{
    for function in &module.callable_defs {
        visitor.visit_fn(function);
    }
}

pub fn walk_fn<V, P>(visitor: &mut V, func: &BlockPyFunction<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
{
    for block in &func.blocks {
        visitor.visit_block(block);
    }
}

pub fn walk_block<V, P>(visitor: &mut V, block: &PassBlock<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
{
    for stmt in &block.body {
        let stmt = stmt.clone().into_stmt();
        visitor.visit_stmt(&stmt);
    }
    if let Some(exc_edge) = &block.exc_edge {
        visitor.visit_label(&exc_edge.target);
    }
    visitor.visit_term(&block.term);
}

pub fn walk_fragment<V, P>(
    visitor: &mut V,
    fragment: &BlockPyCfgFragment<BlockPyStmt<PassExpr<P>>, BlockPyTerm<PassExpr<P>>>,
) where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
{
    for stmt in &fragment.body {
        visitor.visit_stmt(stmt);
    }
    if let Some(term) = &fragment.term {
        visitor.visit_term(term);
    }
}

pub fn walk_stmt<V, P>(visitor: &mut V, stmt: &BlockPyStmt<PassExpr<P>>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
{
    match stmt {
        BlockPyStmt::Assign(assign) => visitor.visit_expr(&assign.value),
        BlockPyStmt::Expr(expr) => visitor.visit_expr(expr),
        BlockPyStmt::Delete(_) => {}
        BlockPyStmt::If(if_stmt) => {
            visitor.visit_expr(&if_stmt.test);
            visitor.visit_fragment(&if_stmt.body);
            visitor.visit_fragment(&if_stmt.orelse);
        }
    }
}

pub fn walk_label<V, P>(visitor: &mut V, label: &BlockPyLabel)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
{
    let _ = visitor;
    let _ = label;
}

pub fn walk_term<V, P>(visitor: &mut V, term: &BlockPyTerm<PassExpr<P>>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
{
    match term {
        BlockPyTerm::Jump(edge) => {
            visitor.visit_label(&edge.target);
        }
        BlockPyTerm::IfTerm(if_term) => {
            visitor.visit_expr(&if_term.test);
            visitor.visit_label(&if_term.then_label);
            visitor.visit_label(&if_term.else_label);
        }
        BlockPyTerm::BranchTable(branch) => {
            visitor.visit_expr(&branch.index);
            for target in &branch.targets {
                visitor.visit_label(target);
            }
            visitor.visit_label(&branch.default_label);
        }
        BlockPyTerm::Raise(raise_stmt) => {
            if let Some(exc) = &raise_stmt.exc {
                visitor.visit_expr(exc);
            }
        }
        BlockPyTerm::Return(value) => visitor.visit_expr(value),
    }
}

pub trait BlockPyModuleMap<PIn, POut>
where
    PIn: BlockPyPass,
    POut: BlockPyPass,
    BlockPyStmt<POut::Expr>: Into<POut::Stmt>,
{
    fn map_module(&self, module: BlockPyModule<PIn>) -> BlockPyModule<POut> {
        BlockPyModule {
            callable_defs: module
                .callable_defs
                .into_iter()
                .map(|function| self.map_fn(function))
                .collect(),
        }
    }

    fn map_fn(&self, func: BlockPyFunction<PIn>) -> BlockPyFunction<POut> {
        BlockPyFunction {
            function_id: func.function_id,
            names: func.names,
            kind: func.kind,
            params: func.params,
            blocks: func
                .blocks
                .into_iter()
                .map(|block| self.map_block(block))
                .collect(),
            doc: func.doc,
            closure_layout: func.closure_layout,
            facts: func.facts,
            try_regions: func.try_regions,
        }
    }

    fn map_block(&self, block: PassBlock<PIn>) -> PassBlock<POut> {
        CfgBlock {
            label: block.label,
            body: block
                .body
                .into_iter()
                .map(|stmt| self.map_stmt(stmt.into_stmt()).into())
                .collect(),
            term: self.map_term(block.term),
            params: block.params,
            exc_edge: block.exc_edge,
        }
    }

    fn map_fragment(
        &self,
        fragment: BlockPyCfgFragment<BlockPyStmt<PassExpr<PIn>>, BlockPyTerm<PassExpr<PIn>>>,
    ) -> BlockPyCfgFragment<BlockPyStmt<PassExpr<POut>>, BlockPyTerm<PassExpr<POut>>> {
        BlockPyCfgFragment {
            body: fragment
                .body
                .into_iter()
                .map(|stmt| self.map_stmt(stmt))
                .collect(),
            term: fragment.term.map(|term| self.map_term(term)),
        }
    }

    fn map_stmt(&self, stmt: BlockPyStmt<PassExpr<PIn>>) -> BlockPyStmt<PassExpr<POut>> {
        match stmt {
            BlockPyStmt::Assign(assign) => BlockPyStmt::Assign(BlockPyAssign {
                target: assign.target,
                value: self.map_expr(assign.value),
            }),
            BlockPyStmt::Expr(expr) => BlockPyStmt::Expr(self.map_expr(expr)),
            BlockPyStmt::Delete(delete) => BlockPyStmt::Delete(delete),
            BlockPyStmt::If(if_stmt) => BlockPyStmt::If(BlockPyIf {
                test: self.map_expr(if_stmt.test),
                body: self.map_fragment(if_stmt.body),
                orelse: self.map_fragment(if_stmt.orelse),
            }),
        }
    }

    fn map_term(&self, term: BlockPyTerm<PassExpr<PIn>>) -> BlockPyTerm<PassExpr<POut>> {
        match term {
            BlockPyTerm::Jump(edge) => BlockPyTerm::Jump(BlockPyEdge {
                target: edge.target,
                args: edge
                    .args
                    .into_iter()
                    .map(|arg| match arg {
                        BlockArg::Name(name) => BlockArg::Name(name),
                        BlockArg::None => BlockArg::None,
                        BlockArg::CurrentException => BlockArg::CurrentException,
                        BlockArg::AbruptKind(kind) => BlockArg::AbruptKind(kind),
                    })
                    .collect(),
            }),
            BlockPyTerm::IfTerm(if_term) => BlockPyTerm::IfTerm(BlockPyIfTerm {
                test: self.map_expr(if_term.test),
                then_label: if_term.then_label,
                else_label: if_term.else_label,
            }),
            BlockPyTerm::BranchTable(branch) => BlockPyTerm::BranchTable(BlockPyBranchTable {
                index: self.map_expr(branch.index),
                targets: branch.targets,
                default_label: branch.default_label,
            }),
            BlockPyTerm::Raise(raise_stmt) => BlockPyTerm::Raise(BlockPyRaise {
                exc: raise_stmt.exc.map(|exc| self.map_expr(exc)),
            }),
            BlockPyTerm::Return(value) => BlockPyTerm::Return(self.map_expr(value)),
        }
    }

    fn map_expr(&self, expr: PassExpr<PIn>) -> PassExpr<POut>;
}

impl<E> BlockPyJumpTerm<BlockPyLabel> for BlockPyTerm<E> {
    fn jump_term(target: BlockPyLabel) -> Self {
        Self::Jump(BlockPyEdge::new(target))
    }
}

impl ImplicitNoneExpr for Expr {
    fn implicit_none_expr() -> Self {
        py_expr!("__dp_NONE")
    }

    fn is_implicit_none_expr(expr: &Self) -> bool {
        matches!(expr, Expr::Name(name) if name.id.as_str() == "__dp_NONE")
    }
}

impl ImplicitNoneExpr for CoreBlockPyExpr {
    fn implicit_none_expr() -> Self {
        Self::Name(implicit_none_name())
    }

    fn is_implicit_none_expr(expr: &Self) -> bool {
        matches!(expr, CoreBlockPyExpr::Name(name) if name.id.as_str() == "__dp_NONE")
    }
}

impl ImplicitNoneExpr for CoreBlockPyExprWithoutAwait {
    fn implicit_none_expr() -> Self {
        Self::Name(implicit_none_name())
    }

    fn is_implicit_none_expr(expr: &Self) -> bool {
        matches!(expr, CoreBlockPyExprWithoutAwait::Name(name) if name.id.as_str() == "__dp_NONE")
    }
}

impl ImplicitNoneExpr for CoreBlockPyExprWithoutAwaitOrYield {
    fn implicit_none_expr() -> Self {
        Self::Name(implicit_none_name())
    }

    fn is_implicit_none_expr(expr: &Self) -> bool {
        matches!(expr, CoreBlockPyExprWithoutAwaitOrYield::Name(name) if name.id.as_str() == "__dp_NONE")
    }
}

impl<E: ImplicitNoneExpr> BlockPyFallthroughTerm<BlockPyLabel> for BlockPyTerm<E> {
    fn implicit_function_return() -> Self {
        Self::Return(E::implicit_none_expr())
    }
}

impl<PIn> BlockPyModule<PIn>
where
    PIn: BlockPyPass,
{
    pub fn visit_module(&self, visitor: &mut impl BlockPyModuleVisitor<PIn>) {
        visitor.visit_module(self);
    }

    pub fn map_module<POut>(self, mapper: &impl BlockPyModuleMap<PIn, POut>) -> BlockPyModule<POut>
    where
        POut: BlockPyPass,
        BlockPyStmt<POut::Expr>: Into<POut::Stmt>,
    {
        mapper.map_module(self)
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
        block.set_term(BlockPyTerm::Jump(BlockPyLabel::from("after").into()));
        let block = block.finish(None);

        assert_eq!(block.body.len(), 1);
        assert!(matches!(block.body[0], BlockPyStmt::Expr(_)));
        assert!(matches!(block.term, BlockPyTerm::Jump(_)));
    }

    #[test]
    fn block_builder_without_term_uses_implicit_none_return_value() {
        let mut block: BlockPyBlockBuilder<Expr> =
            BlockPyBlockBuilder::new(BlockPyLabel::from("start"));
        block.push_stmt(BlockPyStmt::Expr(py_expr!("x")));
        let block = block.finish(None);

        assert_eq!(block.body.len(), 1);
        assert!(matches!(
            &block.term,
            BlockPyTerm::Return(Expr::Name(name)) if name.id.as_str() == "__dp_NONE"
        ));
    }

    #[test]
    fn stmt_fragment_can_carry_optional_term() {
        let fragment: BlockPyStmtFragment<Expr> = BlockPyStmtFragment::with_term(
            vec![BlockPyStmt::Expr(py_expr!("x"))],
            Some(BlockPyTerm::Return(py_expr!("__dp_NONE"))),
        );

        assert_eq!(fragment.body.len(), 1);
        assert!(matches!(fragment.body[0], BlockPyStmt::Expr(_)));
        assert!(matches!(fragment.term, Some(BlockPyTerm::Return(_))));
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
    fn module_visitor_walks_blockpy_in_evaluation_order() {
        #[derive(Default)]
        struct TraceVisitor {
            trace: Vec<String>,
        }

        impl BlockPyModuleVisitor<RuffBlockPyPass> for TraceVisitor {
            fn visit_module(&mut self, module: &BlockPyModule<RuffBlockPyPass>) {
                self.trace.push("module".to_string());
                walk_module(self, module);
            }

            fn visit_fn(&mut self, func: &BlockPyFunction<RuffBlockPyPass>) {
                self.trace.push(format!("fn:{}", func.names.bind_name));
                walk_fn(self, func);
            }

            fn visit_block(&mut self, block: &PassBlock<RuffBlockPyPass>) {
                self.trace.push(format!("block:{}", block.label));
                walk_block(self, block);
            }

            fn visit_fragment(
                &mut self,
                fragment: &BlockPyCfgFragment<
                    <RuffBlockPyPass as BlockPyPass>::Stmt,
                    BlockPyTerm<PassExpr<RuffBlockPyPass>>,
                >,
            ) {
                self.trace.push("fragment".to_string());
                walk_fragment(self, fragment);
            }

            fn visit_stmt(&mut self, stmt: &BlockPyStmt<PassExpr<RuffBlockPyPass>>) {
                let kind = match stmt {
                    BlockPyStmt::Assign(_) => "assign",
                    BlockPyStmt::Expr(_) => "expr",
                    BlockPyStmt::Delete(_) => "delete",
                    BlockPyStmt::If(_) => "if",
                };
                self.trace.push(format!("stmt:{kind}"));
                walk_stmt(self, stmt);
            }

            fn visit_term(&mut self, term: &BlockPyTerm<PassExpr<RuffBlockPyPass>>) {
                let kind = match term {
                    BlockPyTerm::Jump(_) => "jump",
                    BlockPyTerm::IfTerm(_) => "if",
                    BlockPyTerm::BranchTable(_) => "branch_table",
                    BlockPyTerm::Raise(_) => "raise",
                    BlockPyTerm::Return(_) => "return",
                };
                self.trace.push(format!("term:{kind}"));
                walk_term(self, term);
            }

            fn visit_label(&mut self, label: &BlockPyLabel) {
                self.trace.push(format!("label:{}", label.as_str()));
            }

            fn visit_expr(&mut self, expr: &PassExpr<RuffBlockPyPass>) {
                let Expr::Name(name) = expr else {
                    panic!("expected name expr in visitor trace test");
                };
                self.trace.push(format!("expr:{}", name.id));
            }
        }

        let module = BlockPyModule::<RuffBlockPyPass> {
            callable_defs: vec![BlockPyFunction {
                function_id: FunctionId(0),
                names: FunctionName::new("f", "f", "f", "f"),
                kind: BlockPyFunctionKind::Function,
                params: ParamSpec::default(),
                blocks: vec![
                    CfgBlock {
                        label: BlockPyLabel::from("start"),
                        body: vec![
                            BlockPyStmt::Assign(BlockPyAssign {
                                target: name_expr("target"),
                                value: py_expr!("assign_one"),
                            }),
                            BlockPyStmt::If(BlockPyIf {
                                test: py_expr!("if_test"),
                                body: BlockPyCfgFragment::with_term(
                                    vec![BlockPyStmt::Expr(py_expr!("then_expr"))],
                                    Some(BlockPyTerm::Return(py_expr!("then_return"))),
                                ),
                                orelse: BlockPyCfgFragment::with_term(
                                    vec![BlockPyStmt::Expr(py_expr!("else_expr"))],
                                    Some(BlockPyTerm::Raise(BlockPyRaise {
                                        exc: Some(py_expr!("else_raise")),
                                    })),
                                ),
                            }),
                            BlockPyStmt::Expr(py_expr!("after_if")),
                        ],
                        term: BlockPyTerm::IfTerm(BlockPyIfTerm {
                            test: py_expr!("block_term_test"),
                            then_label: BlockPyLabel::from("then"),
                            else_label: BlockPyLabel::from("else"),
                        }),
                        params: Vec::new(),
                        exc_edge: None,
                    },
                    CfgBlock {
                        label: BlockPyLabel::from("done"),
                        body: vec![BlockPyStmt::Delete(BlockPyDelete {
                            target: name_expr("trash"),
                        })],
                        term: BlockPyTerm::Return(py_expr!("final_return")),
                        params: Vec::new(),
                        exc_edge: None,
                    },
                ],
                doc: None,
                closure_layout: None,
                facts: BlockPyCallableFacts::default(),
                try_regions: Vec::new(),
            }],
        };

        let mut visitor = TraceVisitor::default();
        module.visit_module(&mut visitor);

        assert_eq!(
            visitor.trace,
            vec![
                "module",
                "fn:f",
                "block:start",
                "stmt:assign",
                "expr:assign_one",
                "stmt:if",
                "expr:if_test",
                "fragment",
                "stmt:expr",
                "expr:then_expr",
                "term:return",
                "expr:then_return",
                "fragment",
                "stmt:expr",
                "expr:else_expr",
                "term:raise",
                "expr:else_raise",
                "stmt:expr",
                "expr:after_if",
                "term:if",
                "expr:block_term_test",
                "label:then",
                "label:else",
                "block:done",
                "stmt:delete",
                "term:return",
                "expr:final_return",
            ]
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>()
        );
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
        let term = BlockPyTerm::Return(CoreBlockPyExprWithoutAwait::Call(CoreBlockPyCall {
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
        }));

        assert!(BlockPyTerm::<CoreBlockPyExprWithoutAwaitOrYield>::try_from(term).is_err());
    }
}

impl From<CoreBlockPyExpr> for Expr {
    fn from(value: CoreBlockPyExpr) -> Self {
        match value {
            CoreBlockPyExpr::Literal(literal) => core_literal_to_expr(literal),
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
            CoreBlockPyExprWithoutAwait::Literal(literal) => core_literal_to_expr(literal),
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
            CoreBlockPyExprWithoutAwaitOrYield::Literal(literal) => core_literal_to_expr(literal),
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

fn core_literal_to_expr(literal: CoreBlockPyLiteral) -> Expr {
    match literal {
        CoreBlockPyLiteral::StringLiteral(node) => {
            let node_index = node.node_index.clone();
            Expr::StringLiteral(ast::ExprStringLiteral {
                node_index: node_index.clone(),
                range: node.range,
                value: StringLiteralValue::single(StringLiteral {
                    node_index,
                    range: node.range,
                    value: node.value.into(),
                    flags: StringLiteralFlags::empty().with_quote_style(Quote::Double),
                }),
            })
        }
        CoreBlockPyLiteral::BytesLiteral(node) => {
            let node_index = node.node_index.clone();
            Expr::BytesLiteral(ast::ExprBytesLiteral {
                node_index: node_index.clone(),
                range: node.range,
                value: ast::BytesLiteralValue::single(BytesLiteral {
                    node_index,
                    range: node.range,
                    value: node.value.into(),
                    flags: BytesLiteralFlags::empty().with_quote_style(Quote::Double),
                }),
            })
        }
        CoreBlockPyLiteral::NumberLiteral(node) => Expr::NumberLiteral(ast::ExprNumberLiteral {
            node_index: node.node_index,
            range: node.range,
            value: match node.value {
                CoreNumberLiteralValue::Int(value) => ast::Number::Int(value),
                CoreNumberLiteralValue::Float(value) => ast::Number::Float(value),
            },
        }),
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
            BlockPyTerm::Jump(target) => Ok(BlockPyTerm::Jump(BlockPyEdge {
                target: target.target,
                args: target
                    .args
                    .into_iter()
                    .map(|arg| -> Result<BlockArg, CoreBlockPyExpr> {
                        match arg {
                            BlockArg::Name(name) => Ok(BlockArg::Name(name)),
                            BlockArg::None => Ok(BlockArg::None),
                            BlockArg::CurrentException => Ok(BlockArg::CurrentException),
                            BlockArg::AbruptKind(kind) => Ok(BlockArg::AbruptKind(kind)),
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            })),
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
            BlockPyTerm::Return(value) => Ok(BlockPyTerm::Return(value.try_into()?)),
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

impl TryFrom<CfgBlock<BlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>>>
    for CfgBlock<BlockPyStmt<CoreBlockPyExprWithoutAwait>, BlockPyTerm<CoreBlockPyExprWithoutAwait>>
{
    type Error = CoreBlockPyExpr;

    fn try_from(
        value: CfgBlock<BlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>>,
    ) -> Result<Self, Self::Error> {
        Ok(CfgBlock {
            label: value.label,
            body: value
                .body
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
            term: value.term.try_into()?,
            params: value.params,
            exc_edge: value.exc_edge,
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
            BlockPyTerm::Jump(target) => Ok(BlockPyTerm::Jump(BlockPyEdge {
                target: target.target,
                args: target
                    .args
                    .into_iter()
                    .map(|arg| -> Result<BlockArg, CoreBlockPyExprWithoutAwait> {
                        match arg {
                            BlockArg::Name(name) => Ok(BlockArg::Name(name)),
                            BlockArg::None => Ok(BlockArg::None),
                            BlockArg::CurrentException => Ok(BlockArg::CurrentException),
                            BlockArg::AbruptKind(kind) => Ok(BlockArg::AbruptKind(kind)),
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            })),
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
            BlockPyTerm::Return(value) => Ok(BlockPyTerm::Return(value.try_into()?)),
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

impl
    TryFrom<
        CfgBlock<
            BlockPyStmt<CoreBlockPyExprWithoutAwait>,
            BlockPyTerm<CoreBlockPyExprWithoutAwait>,
        >,
    >
    for CfgBlock<
        BlockPyStmt<CoreBlockPyExprWithoutAwaitOrYield>,
        BlockPyTerm<CoreBlockPyExprWithoutAwaitOrYield>,
    >
{
    type Error = CoreBlockPyExprWithoutAwait;

    fn try_from(
        value: CfgBlock<
            BlockPyStmt<CoreBlockPyExprWithoutAwait>,
            BlockPyTerm<CoreBlockPyExprWithoutAwait>,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(CfgBlock {
            label: value.label,
            body: value
                .body
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<_, _>>()?,
            term: value.term.try_into()?,
            params: value.params,
            exc_edge: value.exc_edge,
        })
    }
}

impl TryFrom<BlockPyFunction<CoreBlockPyPassWithoutAwait>>
    for BlockPyFunction<CoreBlockPyPassWithoutAwaitOrYield>
{
    type Error = CoreBlockPyExprWithoutAwait;

    fn try_from(value: BlockPyFunction<CoreBlockPyPassWithoutAwait>) -> Result<Self, Self::Error> {
        let BlockPyFunction {
            function_id,
            names,
            kind,
            params,
            blocks,
            doc,
            closure_layout,
            facts,
            try_regions,
        } = value;
        Ok(BlockPyFunction {
            function_id,
            names,
            kind,
            params,
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
