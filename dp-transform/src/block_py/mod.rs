pub use self::param_specs::ParamKind;
use self::param_specs::ParamSpec;
use crate::passes::ast_to_ast::scope_helpers::cell_name;
use crate::passes::{BbBlockPyPass, PreparedBbBlockPyPass};
use crate::py_expr;
pub use ruff_python_ast::Expr;
use ruff_python_ast::{self as ast, ExprName};
use std::collections::{HashMap, HashSet};
use std::fmt;

pub(crate) mod cfg;
mod convert;
pub(crate) mod dataflow;
pub(crate) mod exception;
pub mod intrinsics;
mod name_gen;
pub(crate) mod param_specs;
pub mod pretty;
pub(crate) mod state;
pub(crate) use convert::{map_call_args_with, map_intrinsic_args_with, map_keyword_args_with};
pub use convert::{BlockPyModuleMap, BlockPyModuleTryMap};
pub use name_gen::{FunctionNameGen, ModuleNameGen};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockPyLabel(pub String);

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FunctionId(pub usize);

impl FunctionId {
    pub fn plan_qualname(self, qualname: &str) -> String {
        format!("{qualname}::__dp_fn_{}", self.0)
    }
}

fn is_internal_symbol(name: &str) -> bool {
    name.starts_with("_dp_") || name.starts_with("__dp_") || name == "__dp__"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NameLocation {
    Local { slot: u32 },
    Global,
    OwnedCell { slot: u32 },
    ClosureCell { slot: u32 },
    CapturedCellSource { slot: u32 },
}

pub trait BlockPyNameLike: Clone + fmt::Debug + From<ast::ExprName> + Into<ast::ExprName> {
    fn id_str(&self) -> &str;
    fn range(&self) -> ruff_text_size::TextRange;
    fn node_index(&self) -> ast::AtomicNodeIndex;
}

#[derive(Debug, Clone)]
pub struct RuffExpr(pub ast::Expr);

impl From<ast::Expr> for RuffExpr {
    fn from(value: ast::Expr) -> Self {
        Self(value)
    }
}

impl From<RuffExpr> for ast::Expr {
    fn from(value: RuffExpr) -> Self {
        value.0
    }
}

pub trait MapExpr<T>: Clone + fmt::Debug + Into<Expr> + Sized {
    fn map_expr(self, f: &mut impl FnMut(Self) -> T) -> T;
}

pub trait BlockPyExprLike: Clone + fmt::Debug + Into<Expr> + MapExpr<Self> {
    fn walk_child_exprs<F>(&self, f: &mut F)
    where
        F: FnMut(&Self),
    {
        let _ = self.clone().map_expr(&mut |child| {
            f(&child);
            child
        });
    }
}

impl BlockPyNameLike for ast::ExprName {
    fn id_str(&self) -> &str {
        self.id.as_str()
    }

    fn range(&self) -> ruff_text_size::TextRange {
        self.range
    }

    fn node_index(&self) -> ast::AtomicNodeIndex {
        self.node_index.clone()
    }
}

impl MapExpr<Expr> for Expr {
    fn map_expr(self, f: &mut impl FnMut(Self) -> Expr) -> Expr {
        struct DirectChildTransformer<'a, F>(&'a mut F);

        impl<F> crate::transformer::Transformer for DirectChildTransformer<'_, F>
        where
            F: FnMut(Expr) -> Expr,
        {
            fn visit_expr(&mut self, expr: &mut Expr) {
                *expr = (self.0)(expr.clone());
            }
        }

        let mut expr = self;
        let mut transformer = DirectChildTransformer(f);
        crate::transformer::walk_expr(&mut transformer, &mut expr);
        expr
    }
}

impl MapExpr<RuffExpr> for RuffExpr {
    fn map_expr(self, f: &mut impl FnMut(Self) -> RuffExpr) -> RuffExpr {
        RuffExpr(self.0.map_expr(&mut |expr| f(RuffExpr(expr)).0))
    }
}

impl<T> BlockPyExprLike for T where T: Clone + fmt::Debug + Into<Expr> + MapExpr<Self> {}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LocatedName {
    pub id: ruff_python_ast::name::Name,
    pub ctx: ast::ExprContext,
    pub range: ruff_text_size::TextRange,
    pub node_index: ast::AtomicNodeIndex,
    pub location: NameLocation,
}

impl LocatedName {
    pub fn with_location(mut self, location: NameLocation) -> Self {
        self.location = location;
        self
    }
}

impl BlockPyNameLike for LocatedName {
    fn id_str(&self) -> &str {
        self.id.as_str()
    }

    fn range(&self) -> ruff_text_size::TextRange {
        self.range
    }

    fn node_index(&self) -> ast::AtomicNodeIndex {
        self.node_index.clone()
    }
}

impl From<ast::ExprName> for LocatedName {
    fn from(value: ast::ExprName) -> Self {
        Self {
            id: value.id,
            ctx: value.ctx,
            range: value.range,
            node_index: value.node_index,
            location: NameLocation::Global,
        }
    }
}

impl From<LocatedName> for ast::ExprName {
    fn from(value: LocatedName) -> Self {
        Self {
            id: value.id,
            ctx: value.ctx,
            range: value.range,
            node_index: value.node_index,
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BindingTarget {
    Local,
    ModuleGlobal,
    ClassNamespace,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockPyCellBindingKind {
    Owner,
    Capture,
}

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum BlockPyBindingKind {
    #[default]
    Local,
    Global,
    Cell(BlockPyCellBindingKind),
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
    RuntimeAbruptKindFallthrough,
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

pub(crate) fn move_entry_block_to_front<S, T>(blocks: &mut Vec<CfgBlock<S, T>>, entry_label: &str) {
    if let Some(entry_index) = blocks
        .iter()
        .position(|block| block.label.as_str() == entry_label)
    {
        if entry_index != 0 {
            let entry_block = blocks.remove(entry_index);
            blocks.insert(0, entry_block);
        }
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
pub enum CoreBlockPyExprWithAwaitAndYield {
    Name(ast::ExprName),
    Literal(CoreBlockPyLiteral),
    Call(CoreBlockPyCall<CoreBlockPyExprWithAwaitAndYield>),
    Intrinsic(IntrinsicCall<CoreBlockPyExprWithAwaitAndYield>),
    Await(CoreBlockPyAwait<CoreBlockPyExprWithAwaitAndYield>),
    Yield(CoreBlockPyYield<CoreBlockPyExprWithAwaitAndYield>),
    YieldFrom(CoreBlockPyYieldFrom<CoreBlockPyExprWithAwaitAndYield>),
}

#[derive(Debug, Clone)]
pub enum CoreBlockPyExprWithYield {
    Name(ast::ExprName),
    Literal(CoreBlockPyLiteral),
    Call(CoreBlockPyCall<CoreBlockPyExprWithYield>),
    Intrinsic(IntrinsicCall<CoreBlockPyExprWithYield>),
    Yield(CoreBlockPyYield<CoreBlockPyExprWithYield>),
    YieldFrom(CoreBlockPyYieldFrom<CoreBlockPyExprWithYield>),
}

#[derive(Debug, Clone)]
pub enum CoreBlockPyExpr<N = ast::ExprName> {
    Name(N),
    Literal(CoreBlockPyLiteral),
    Call(CoreBlockPyCall<CoreBlockPyExpr<N>>),
    Intrinsic(IntrinsicCall<CoreBlockPyExpr<N>>),
}

pub type LocatedCoreBlockPyExpr = CoreBlockPyExpr<LocatedName>;

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
pub struct IntrinsicCall<E> {
    pub intrinsic: &'static dyn intrinsics::Intrinsic,
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub args: Vec<E>,
}

pub(crate) trait CoreCallLikeExpr: Sized {
    fn from_name(name: ast::ExprName) -> Self;

    fn from_call(call: CoreBlockPyCall<Self>) -> Self;

    fn from_intrinsic(call: IntrinsicCall<Self>) -> Self;
}

impl CoreCallLikeExpr for CoreBlockPyExprWithAwaitAndYield {
    fn from_name(name: ast::ExprName) -> Self {
        Self::Name(name)
    }

    fn from_call(call: CoreBlockPyCall<Self>) -> Self {
        Self::Call(call)
    }

    fn from_intrinsic(call: IntrinsicCall<Self>) -> Self {
        Self::Intrinsic(call)
    }
}

impl MapExpr<CoreBlockPyExprWithAwaitAndYield> for CoreBlockPyExprWithAwaitAndYield {
    fn map_expr(
        self,
        f: &mut impl FnMut(Self) -> CoreBlockPyExprWithAwaitAndYield,
    ) -> CoreBlockPyExprWithAwaitAndYield {
        match self {
            Self::Name(_) | Self::Literal(_) => self,
            Self::Call(call) => Self::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(f(*call.func)),
                args: map_call_args_with(call.args, &mut *f),
                keywords: map_keyword_args_with(call.keywords, &mut *f),
            }),
            Self::Intrinsic(call) => Self::Intrinsic(IntrinsicCall {
                intrinsic: call.intrinsic,
                node_index: call.node_index,
                range: call.range,
                args: map_intrinsic_args_with(call.args, &mut *f),
            }),
            Self::Await(await_expr) => Self::Await(CoreBlockPyAwait {
                node_index: await_expr.node_index,
                range: await_expr.range,
                value: Box::new(f(*await_expr.value)),
            }),
            Self::Yield(yield_expr) => Self::Yield(CoreBlockPyYield {
                node_index: yield_expr.node_index,
                range: yield_expr.range,
                value: yield_expr.value.map(|value| Box::new(f(*value))),
            }),
            Self::YieldFrom(yield_from_expr) => Self::YieldFrom(CoreBlockPyYieldFrom {
                node_index: yield_from_expr.node_index,
                range: yield_from_expr.range,
                value: Box::new(f(*yield_from_expr.value)),
            }),
        }
    }
}

impl CoreCallLikeExpr for CoreBlockPyExprWithYield {
    fn from_name(name: ast::ExprName) -> Self {
        Self::Name(name)
    }

    fn from_call(call: CoreBlockPyCall<Self>) -> Self {
        Self::Call(call)
    }

    fn from_intrinsic(call: IntrinsicCall<Self>) -> Self {
        Self::Intrinsic(call)
    }
}

impl MapExpr<CoreBlockPyExprWithYield> for CoreBlockPyExprWithYield {
    fn map_expr(
        self,
        f: &mut impl FnMut(Self) -> CoreBlockPyExprWithYield,
    ) -> CoreBlockPyExprWithYield {
        match self {
            Self::Name(_) | Self::Literal(_) => self,
            Self::Call(call) => Self::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(f(*call.func)),
                args: map_call_args_with(call.args, &mut *f),
                keywords: map_keyword_args_with(call.keywords, &mut *f),
            }),
            Self::Intrinsic(call) => Self::Intrinsic(IntrinsicCall {
                intrinsic: call.intrinsic,
                node_index: call.node_index,
                range: call.range,
                args: map_intrinsic_args_with(call.args, &mut *f),
            }),
            Self::Yield(yield_expr) => Self::Yield(CoreBlockPyYield {
                node_index: yield_expr.node_index,
                range: yield_expr.range,
                value: yield_expr.value.map(|value| Box::new(f(*value))),
            }),
            Self::YieldFrom(yield_from_expr) => Self::YieldFrom(CoreBlockPyYieldFrom {
                node_index: yield_from_expr.node_index,
                range: yield_from_expr.range,
                value: Box::new(f(*yield_from_expr.value)),
            }),
        }
    }
}

impl<N: From<ast::ExprName>> CoreCallLikeExpr for CoreBlockPyExpr<N> {
    fn from_name(name: ast::ExprName) -> Self {
        Self::Name(name.into())
    }

    fn from_call(call: CoreBlockPyCall<Self>) -> Self {
        Self::Call(call)
    }

    fn from_intrinsic(call: IntrinsicCall<Self>) -> Self {
        Self::Intrinsic(call)
    }
}

impl<NIn, NOut> MapExpr<CoreBlockPyExpr<NOut>> for CoreBlockPyExpr<NIn>
where
    NIn: BlockPyNameLike,
    NOut: BlockPyNameLike + From<NIn>,
{
    fn map_expr(self, f: &mut impl FnMut(Self) -> CoreBlockPyExpr<NOut>) -> CoreBlockPyExpr<NOut> {
        match self {
            Self::Name(name) => CoreBlockPyExpr::Name(name.into().into()),
            Self::Literal(literal) => CoreBlockPyExpr::Literal(literal),
            Self::Call(call) => CoreBlockPyExpr::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(f(*call.func)),
                args: map_call_args_with(call.args, &mut *f),
                keywords: map_keyword_args_with(call.keywords, &mut *f),
            }),
            Self::Intrinsic(call) => CoreBlockPyExpr::Intrinsic(IntrinsicCall {
                intrinsic: call.intrinsic,
                node_index: call.node_index,
                range: call.range,
                args: map_intrinsic_args_with(call.args, &mut *f),
            }),
        }
    }
}

pub(crate) fn core_call_expr_with_meta<E: CoreCallLikeExpr>(
    func: E,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<CoreBlockPyCallArg<E>>,
    keywords: Vec<CoreBlockPyKeywordArg<E>>,
) -> E {
    E::from_call(CoreBlockPyCall {
        node_index,
        range,
        func: Box::new(func),
        args,
        keywords,
    })
}

pub(crate) fn core_named_call_expr_with_meta<E: CoreCallLikeExpr>(
    func_name: &str,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<CoreBlockPyCallArg<E>>,
    keywords: Vec<CoreBlockPyKeywordArg<E>>,
) -> E {
    core_call_expr_with_meta(
        E::from_name(ExprName {
            id: func_name.into(),
            ctx: ast::ExprContext::Load,
            range,
            node_index: node_index.clone(),
        }),
        node_index,
        range,
        args,
        keywords,
    )
}

pub(crate) fn core_intrinsic_expr_with_meta<E: CoreCallLikeExpr>(
    intrinsic: &'static dyn intrinsics::Intrinsic,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<E>,
) -> E {
    E::from_intrinsic(IntrinsicCall {
        intrinsic,
        node_index,
        range,
        args,
    })
}

pub(crate) fn core_positional_intrinsic_expr_with_meta<E: CoreCallLikeExpr>(
    intrinsic: &'static dyn intrinsics::Intrinsic,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<E>,
) -> E {
    core_intrinsic_expr_with_meta(intrinsic, node_index, range, args)
}

pub(crate) fn core_positional_call_expr_with_meta<E: CoreCallLikeExpr>(
    func_name: &str,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<E>,
) -> E {
    core_named_call_expr_with_meta(
        func_name,
        node_index,
        range,
        args.into_iter()
            .map(CoreBlockPyCallArg::Positional)
            .collect(),
        Vec::new(),
    )
}

#[derive(Debug, Clone)]
pub enum CoreBlockPyCallArg<E = CoreBlockPyExprWithAwaitAndYield> {
    Positional(E),
    Starred(E),
}

impl<E> CoreBlockPyCallArg<E> {
    pub fn expr(&self) -> &E {
        match self {
            Self::Positional(expr) | Self::Starred(expr) => expr,
        }
    }

    pub fn expr_mut(&mut self) -> &mut E {
        match self {
            Self::Positional(expr) | Self::Starred(expr) => expr,
        }
    }

    pub fn map_expr<T>(self, f: impl FnOnce(E) -> T) -> CoreBlockPyCallArg<T> {
        match self {
            Self::Positional(expr) => CoreBlockPyCallArg::Positional(f(expr)),
            Self::Starred(expr) => CoreBlockPyCallArg::Starred(f(expr)),
        }
    }

    pub fn try_map_expr<T, Error>(
        self,
        f: impl FnOnce(E) -> Result<T, Error>,
    ) -> Result<CoreBlockPyCallArg<T>, Error> {
        match self {
            Self::Positional(expr) => f(expr).map(CoreBlockPyCallArg::Positional),
            Self::Starred(expr) => f(expr).map(CoreBlockPyCallArg::Starred),
        }
    }
}

#[derive(Debug, Clone)]
pub enum CoreBlockPyKeywordArg<E = CoreBlockPyExprWithAwaitAndYield> {
    Named { arg: ast::Identifier, value: E },
    Starred(E),
}

impl<E> CoreBlockPyKeywordArg<E> {
    pub fn expr(&self) -> &E {
        match self {
            Self::Named { value, .. } | Self::Starred(value) => value,
        }
    }

    pub fn expr_mut(&mut self) -> &mut E {
        match self {
            Self::Named { value, .. } | Self::Starred(value) => value,
        }
    }

    pub fn map_expr<T>(self, f: impl FnOnce(E) -> T) -> CoreBlockPyKeywordArg<T> {
        match self {
            Self::Named { arg, value } => CoreBlockPyKeywordArg::Named {
                arg,
                value: f(value),
            },
            Self::Starred(value) => CoreBlockPyKeywordArg::Starred(f(value)),
        }
    }

    pub fn try_map_expr<T, Error>(
        self,
        f: impl FnOnce(E) -> Result<T, Error>,
    ) -> Result<CoreBlockPyKeywordArg<T>, Error> {
        match self {
            Self::Named { arg, value } => {
                f(value).map(|value| CoreBlockPyKeywordArg::Named { arg, value })
            }
            Self::Starred(value) => f(value).map(CoreBlockPyKeywordArg::Starred),
        }
    }
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

#[derive(Debug, Clone, Default)]
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

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub enum BlockPyCallableScopeKind {
    #[default]
    Function,
    Class,
    Module,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockPyClassBodyFallback {
    Global,
    Cell,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockPyEffectiveBinding {
    Local,
    Global,
    Cell(BlockPyCellBindingKind),
    ClassBody(BlockPyClassBodyFallback),
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum BlockPyBindingPurpose {
    Load,
    Store,
}

#[derive(Debug, Clone, Default)]
pub struct BlockPyCallableSemanticInfo {
    pub names: FunctionName,
    pub scope_kind: BlockPyCallableScopeKind,
    pub bindings: HashMap<String, BlockPyBindingKind>,
    pub local_defs: HashSet<String>,
    pub cell_storage_names: HashMap<String, String>,
    pub cell_capture_source_names: HashMap<String, String>,
    pub owned_cell_source_names: HashSet<String>,
    pub semantic_internal_names: HashSet<String>,
    pub type_param_names: HashSet<String>,
    pub effective_load_bindings: HashMap<String, BlockPyEffectiveBinding>,
    pub effective_store_bindings: HashMap<String, BlockPyEffectiveBinding>,
}

pub(crate) fn derive_effective_binding_for_name(
    name: &str,
    binding: BlockPyBindingKind,
    scope_kind: BlockPyCallableScopeKind,
    type_param_names: &HashSet<String>,
    purpose: BlockPyBindingPurpose,
    honor_internal_name: bool,
) -> BlockPyEffectiveBinding {
    if is_internal_symbol(name) && !honor_internal_name {
        return BlockPyEffectiveBinding::Local;
    }
    match purpose {
        BlockPyBindingPurpose::Load => match (scope_kind, binding) {
            (BlockPyCallableScopeKind::Class, BlockPyBindingKind::Cell(_)) => {
                BlockPyEffectiveBinding::ClassBody(BlockPyClassBodyFallback::Cell)
            }
            (BlockPyCallableScopeKind::Class, BlockPyBindingKind::Local)
            | (BlockPyCallableScopeKind::Class, BlockPyBindingKind::Global) => {
                BlockPyEffectiveBinding::ClassBody(BlockPyClassBodyFallback::Global)
            }
            (_, BlockPyBindingKind::Global) => BlockPyEffectiveBinding::Global,
            (_, BlockPyBindingKind::Cell(kind)) => BlockPyEffectiveBinding::Cell(kind),
            (_, BlockPyBindingKind::Local) => BlockPyEffectiveBinding::Local,
        },
        BlockPyBindingPurpose::Store => {
            if scope_kind == BlockPyCallableScopeKind::Class && type_param_names.contains(name) {
                return match binding {
                    BlockPyBindingKind::Local => BlockPyEffectiveBinding::Local,
                    BlockPyBindingKind::Global => BlockPyEffectiveBinding::Global,
                    BlockPyBindingKind::Cell(kind) => BlockPyEffectiveBinding::Cell(kind),
                };
            }
            match (scope_kind, binding) {
                (BlockPyCallableScopeKind::Class, BlockPyBindingKind::Local) => {
                    BlockPyEffectiveBinding::ClassBody(BlockPyClassBodyFallback::Global)
                }
                (_, BlockPyBindingKind::Global) => BlockPyEffectiveBinding::Global,
                (_, BlockPyBindingKind::Cell(kind)) => BlockPyEffectiveBinding::Cell(kind),
                (_, BlockPyBindingKind::Local) => BlockPyEffectiveBinding::Local,
            }
        }
    }
}

impl BlockPyCallableSemanticInfo {
    pub fn honors_internal_binding(&self, name: &str) -> bool {
        !is_internal_symbol(name) || self.semantic_internal_names.contains(name)
    }

    pub fn binding_kind(&self, name: &str) -> Option<BlockPyBindingKind> {
        self.bindings.get(name).copied()
    }

    pub fn has_local_def(&self, name: &str) -> bool {
        self.local_defs.contains(name)
    }

    pub fn effective_binding(
        &self,
        name: &str,
        purpose: BlockPyBindingPurpose,
    ) -> Option<BlockPyEffectiveBinding> {
        match purpose {
            BlockPyBindingPurpose::Load => self.effective_load_bindings.get(name).copied(),
            BlockPyBindingPurpose::Store => self.effective_store_bindings.get(name).copied(),
        }
    }

    pub fn insert_binding(
        &mut self,
        name: impl Into<String>,
        binding: BlockPyBindingKind,
        honor_internal_name: bool,
        cell_storage_name: Option<String>,
    ) {
        let name = name.into();
        self.bindings.insert(name.clone(), binding);
        if let Some(cell_storage_name) = cell_storage_name {
            self.cell_storage_names
                .insert(name.clone(), cell_storage_name.clone());
            self.cell_capture_source_names
                .insert(name.clone(), cell_storage_name);
        }
        if honor_internal_name {
            self.semantic_internal_names.insert(name.clone());
        }
        self.effective_load_bindings.insert(
            name.clone(),
            derive_effective_binding_for_name(
                name.as_str(),
                binding,
                self.scope_kind,
                &self.type_param_names,
                BlockPyBindingPurpose::Load,
                honor_internal_name,
            ),
        );
        self.effective_store_bindings.insert(
            name.clone(),
            derive_effective_binding_for_name(
                name.as_str(),
                binding,
                self.scope_kind,
                &self.type_param_names,
                BlockPyBindingPurpose::Store,
                honor_internal_name,
            ),
        );
    }

    pub fn resolved_load_binding_kind(&self, name: &str) -> BlockPyBindingKind {
        if let Some(binding) = self.binding_kind(name) {
            if self.honors_internal_binding(name) {
                return binding;
            }
        }
        if is_internal_symbol(name) {
            return BlockPyBindingKind::Local;
        }
        BlockPyBindingKind::Global
    }

    pub fn is_cell_binding(&self, name: &str) -> bool {
        matches!(self.binding_kind(name), Some(BlockPyBindingKind::Cell(_)))
    }

    pub fn cell_storage_name(&self, name: &str) -> String {
        self.cell_storage_names
            .get(name)
            .cloned()
            .unwrap_or_else(|| cell_name(name))
    }

    pub fn cell_capture_source_name(&self, name: &str) -> String {
        self.cell_capture_source_names
            .get(name)
            .cloned()
            .unwrap_or_else(|| cell_name(name))
    }

    pub fn cell_ref_source_name(&self, name: &str) -> String {
        if self.is_cell_binding(name) {
            self.cell_storage_name(name)
        } else {
            self.cell_capture_source_name(name)
        }
    }

    pub fn logical_name_for_cell_capture_source(&self, storage_name: &str) -> Option<String> {
        self.cell_capture_source_names
            .iter()
            .find_map(|(logical_name, current_storage_name)| {
                (current_storage_name == storage_name).then(|| logical_name.clone())
            })
            .or_else(|| self.logical_name_for_cell_storage(storage_name))
    }

    pub fn binding_target_for_name(
        &self,
        name: &str,
        purpose: BlockPyBindingPurpose,
    ) -> BindingTarget {
        if let Some(binding) = self.effective_binding(name, purpose) {
            if self.honors_internal_binding(name) {
                return match binding {
                    BlockPyEffectiveBinding::Global => BindingTarget::ModuleGlobal,
                    BlockPyEffectiveBinding::ClassBody(_) => BindingTarget::ClassNamespace,
                    BlockPyEffectiveBinding::Local | BlockPyEffectiveBinding::Cell(_) => {
                        BindingTarget::Local
                    }
                };
            }
        }
        if is_internal_symbol(name) {
            return BindingTarget::Local;
        }
        match self.effective_binding(name, purpose) {
            Some(BlockPyEffectiveBinding::Global) => BindingTarget::ModuleGlobal,
            Some(BlockPyEffectiveBinding::ClassBody(_)) => BindingTarget::ClassNamespace,
            _ => BindingTarget::Local,
        }
    }

    pub fn owned_cell_storage_names(&self) -> HashSet<String> {
        let mut names = self
            .bindings
            .iter()
            .filter_map(|(name, binding)| {
                matches!(
                    binding,
                    BlockPyBindingKind::Cell(BlockPyCellBindingKind::Owner)
                )
                .then(|| self.cell_storage_name(name.as_str()))
            })
            .collect::<HashSet<_>>();
        names.extend(self.owned_cell_source_names.iter().cloned());
        names
    }

    pub fn local_cell_storage_names(&self) -> HashSet<String> {
        if !matches!(self.scope_kind, BlockPyCallableScopeKind::Function) {
            return HashSet::new();
        }
        self.owned_cell_storage_names()
    }

    pub fn logical_name_for_cell_storage(&self, storage_name: &str) -> Option<String> {
        if let Some(logical_name) = storage_name.strip_prefix("_dp_cell_") {
            return Some(logical_name.to_string());
        }
        self.cell_storage_names
            .iter()
            .find_map(|(logical_name, current_storage_name)| {
                (current_storage_name == storage_name).then(|| logical_name.clone())
            })
    }
}

#[derive(Debug)]
pub struct BlockPyFunction<P: BlockPyPass> {
    pub function_id: FunctionId,
    pub name_gen: FunctionNameGen,
    pub names: FunctionName,
    pub kind: BlockPyFunctionKind,
    pub params: ParamSpec,
    pub blocks: Vec<CfgBlock<P::Stmt, BlockPyTerm<P::Expr>>>,
    pub doc: Option<String>,
    pub closure_layout: Option<ClosureLayout>,
    pub semantic: BlockPyCallableSemanticInfo,
}

impl<P: BlockPyPass> Clone for BlockPyFunction<P> {
    fn clone(&self) -> Self {
        Self {
            function_id: self.function_id,
            // Share the allocator state so cloned analysis/rendering snapshots
            // cannot accidentally reissue duplicate generated names.
            name_gen: self.name_gen.share(),
            names: self.names.clone(),
            kind: self.kind,
            params: self.params.clone(),
            blocks: self.blocks.clone(),
            doc: self.doc.clone(),
            closure_layout: self.closure_layout.clone(),
            semantic: self.semantic.clone(),
        }
    }
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
            name_gen: self.name_gen,
            names: self.names,
            kind: self.kind,
            params: self.params,
            blocks: self.blocks.into_iter().map(&mut f).collect(),
            doc: self.doc,
            closure_layout: self.closure_layout,
            semantic: self.semantic,
        }
    }
}

pub trait BlockPyNormalizedStmt {
    fn assert_blockpy_normalized(&self);
}

pub trait IntoBlockPyStmt<E, N>: Clone + fmt::Debug {
    fn into_stmt(self) -> BlockPyStmt<E, N>;
}

pub trait BlockPyPass: Clone + fmt::Debug {
    type Name: BlockPyNameLike;
    type Expr: BlockPyExprLike;
    type Stmt: BlockPyNormalizedStmt + IntoBlockPyStmt<Self::Expr, Self::Name>;
}

pub type PassExpr<P> = <P as BlockPyPass>::Expr;
pub type PassName<P> = <P as BlockPyPass>::Name;
pub type StructuredPassStmt<P> = BlockPyStmt<PassExpr<P>, PassName<P>>;
pub type PassBlock<P> = CfgBlock<<P as BlockPyPass>::Stmt, BlockPyTerm<PassExpr<P>>>;
pub type BbBlock = PassBlock<BbBlockPyPass>;
pub type PreparedBbBlock = PassBlock<PreparedBbBlockPyPass>;

pub type BlockPyCfgBlock<S, T> = CfgBlock<S, T>;
pub type BlockPyBlock<E = Expr, N = ExprName> = BlockPyCfgBlock<BlockPyStmt<E, N>, BlockPyTerm<E>>;
pub type BlockPyStructuredIf<E = Expr, N = ExprName> =
    BlockPyIf<E, BlockPyStmt<E, N>, BlockPyTerm<E>>;

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

pub fn expr_any<E, F>(expr: &E, mut predicate: F) -> bool
where
    E: BlockPyExprLike,
    F: FnMut(&E) -> bool,
{
    fn expr_any_impl<E, F>(expr: &E, predicate: &mut F) -> bool
    where
        E: BlockPyExprLike,
        F: FnMut(&E) -> bool,
    {
        if predicate(expr) {
            return true;
        }

        let mut found = false;
        expr.walk_child_exprs(&mut |child| {
            if !found && expr_any_impl(child, predicate) {
                found = true;
            }
        });
        found
    }

    expr_any_impl(expr, &mut predicate)
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

pub type BlockPyStmtFragment<E = Expr, N = ExprName> =
    BlockPyCfgFragment<BlockPyStmt<E, N>, BlockPyTerm<E>>;

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

pub type BlockPyStmtFragmentBuilder<E = Expr, N = ExprName> =
    BlockPyCfgFragmentBuilder<BlockPyStmt<E, N>, BlockPyTerm<E>>;

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

pub type BlockPyBlockBuilder<E = Expr, N = ExprName> =
    BlockPyCfgBlockBuilder<BlockPyStmt<E, N>, BlockPyTerm<E>>;

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
pub enum BlockPyStmt<E = Expr, N = ExprName> {
    Assign(BlockPyAssign<E, N>),
    Expr(E),
    Delete(BlockPyDelete<N>),
    If(BlockPyStructuredIf<E, N>),
}

impl<E: std::fmt::Debug, N: std::fmt::Debug> BlockPyStmt<E, N> {
    pub fn assert_normalized(&self) {
        if let Self::If(if_stmt) = self {
            if_stmt.body.assert_normalized();
            if_stmt.orelse.assert_normalized();
        }
    }
}

impl<E: std::fmt::Debug, N: std::fmt::Debug> BlockPyNormalizedStmt for BlockPyStmt<E, N> {
    fn assert_blockpy_normalized(&self) {
        self.assert_normalized();
    }
}

pub fn convert_blockpy_stmt_expr<EIn, EOut, N>(value: BlockPyStmt<EIn, N>) -> BlockPyStmt<EOut, N>
where
    EOut: From<EIn>,
{
    match value {
        BlockPyStmt::Assign(assign) => BlockPyStmt::Assign(BlockPyAssign {
            target: assign.target,
            value: assign.value.into(),
        }),
        BlockPyStmt::Expr(expr) => BlockPyStmt::Expr(expr.into()),
        BlockPyStmt::Delete(delete) => BlockPyStmt::Delete(delete),
        BlockPyStmt::If(if_stmt) => BlockPyStmt::If(BlockPyIf {
            test: if_stmt.test.into(),
            body: convert_blockpy_fragment_expr(if_stmt.body),
            orelse: convert_blockpy_fragment_expr(if_stmt.orelse),
        }),
    }
}

#[derive(Debug, Clone)]
pub enum BbStmt<E = CoreBlockPyExpr<LocatedName>, N = LocatedName> {
    Assign(BlockPyAssign<E, N>),
    Expr(E),
    Delete(BlockPyDelete<N>),
}

impl<E, N> From<BlockPyAssign<E, N>> for BbStmt<E, N> {
    fn from(value: BlockPyAssign<E, N>) -> Self {
        Self::Assign(value)
    }
}

impl<N> From<CoreBlockPyExpr<N>> for BbStmt<CoreBlockPyExpr<N>, N> {
    fn from(value: CoreBlockPyExpr<N>) -> Self {
        Self::Expr(value)
    }
}

impl<E, N> From<BlockPyDelete<N>> for BbStmt<E, N> {
    fn from(value: BlockPyDelete<N>) -> Self {
        Self::Delete(value)
    }
}

impl<EIn, EOut, N> From<BlockPyStmt<EIn, N>> for BbStmt<EOut, N>
where
    EOut: From<EIn>,
{
    fn from(value: BlockPyStmt<EIn, N>) -> Self {
        match value {
            BlockPyStmt::Assign(assign) => Self::Assign(BlockPyAssign {
                target: assign.target,
                value: assign.value.into(),
            }),
            BlockPyStmt::Expr(expr) => Self::Expr(expr.into()),
            BlockPyStmt::Delete(delete) => Self::Delete(delete),
            BlockPyStmt::If(_) => panic!("structured BlockPy If reached BbStmt conversion"),
        }
    }
}

impl<E: Clone + fmt::Debug, N: Clone + fmt::Debug> IntoBlockPyStmt<E, N> for BbStmt<E, N> {
    fn into_stmt(self) -> BlockPyStmt<E, N> {
        match self {
            BbStmt::Assign(assign) => BlockPyStmt::Assign(assign),
            BbStmt::Expr(expr) => BlockPyStmt::Expr(expr),
            BbStmt::Delete(delete) => BlockPyStmt::Delete(delete),
        }
    }
}

impl<E, N> BlockPyNormalizedStmt for BbStmt<E, N> {
    fn assert_blockpy_normalized(&self) {}
}

impl<E: Clone + fmt::Debug, N: Clone + fmt::Debug> IntoBlockPyStmt<E, N> for BlockPyStmt<E, N> {
    fn into_stmt(self) -> BlockPyStmt<E, N> {
        self
    }
}

#[derive(Debug, Clone)]
pub enum BlockPyTerm<E = Expr> {
    Jump(BlockPyEdge),
    IfTerm(BlockPyIfTerm<E>),
    BranchTable(BlockPyBranchTable<E>),
    Raise(BlockPyRaise<E>),
    Return(E),
}

#[derive(Debug, Clone)]
pub struct BlockPyAssign<E = Expr, N = ExprName> {
    pub target: N,
    pub value: E,
}

#[derive(Debug, Clone)]
pub struct BlockPyDelete<N = ExprName> {
    pub target: N,
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

pub fn convert_blockpy_fragment_expr<EIn, EOut, N>(
    value: BlockPyCfgFragment<BlockPyStmt<EIn, N>, BlockPyTerm<EIn>>,
) -> BlockPyCfgFragment<BlockPyStmt<EOut, N>, BlockPyTerm<EOut>>
where
    EOut: From<EIn>,
{
    BlockPyCfgFragment {
        body: value
            .body
            .into_iter()
            .map(convert_blockpy_stmt_expr)
            .collect(),
        term: value.term.map(convert_blockpy_term_expr),
    }
}

pub fn convert_blockpy_term_expr<EIn, EOut>(value: BlockPyTerm<EIn>) -> BlockPyTerm<EOut>
where
    EOut: From<EIn>,
{
    match value {
        BlockPyTerm::Jump(edge) => BlockPyTerm::Jump(edge),
        BlockPyTerm::IfTerm(if_term) => BlockPyTerm::IfTerm(BlockPyIfTerm {
            test: if_term.test.into(),
            then_label: if_term.then_label,
            else_label: if_term.else_label,
        }),
        BlockPyTerm::BranchTable(branch) => BlockPyTerm::BranchTable(BlockPyBranchTable {
            index: branch.index.into(),
            targets: branch.targets,
            default_label: branch.default_label,
        }),
        BlockPyTerm::Raise(raise_stmt) => BlockPyTerm::Raise(BlockPyRaise {
            exc: raise_stmt.exc.map(Into::into),
        }),
        BlockPyTerm::Return(value) => BlockPyTerm::Return(value.into()),
    }
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
    PassExpr<P>: MapExpr<PassExpr<P>>,
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
        fragment: &BlockPyCfgFragment<StructuredPassStmt<P>, BlockPyTerm<PassExpr<P>>>,
    ) {
        walk_fragment(self, fragment);
    }

    fn visit_stmt(&mut self, stmt: &StructuredPassStmt<P>) {
        walk_stmt(self, stmt);
    }

    fn visit_term(&mut self, term: &BlockPyTerm<PassExpr<P>>) {
        walk_term(self, term);
    }

    fn visit_label(&mut self, label: &BlockPyLabel) {
        walk_label::<Self, P>(self, label);
    }

    fn visit_expr(&mut self, expr: &PassExpr<P>) {
        walk_expr(self, expr);
    }
}

pub fn walk_module<V, P>(visitor: &mut V, module: &BlockPyModule<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
{
    for function in &module.callable_defs {
        visitor.visit_fn(function);
    }
}

pub fn walk_fn<V, P>(visitor: &mut V, func: &BlockPyFunction<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
{
    for block in &func.blocks {
        visitor.visit_block(block);
    }
}

pub fn walk_block<V, P>(visitor: &mut V, block: &PassBlock<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
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
    fragment: &BlockPyCfgFragment<StructuredPassStmt<P>, BlockPyTerm<PassExpr<P>>>,
) where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
{
    for stmt in &fragment.body {
        visitor.visit_stmt(stmt);
    }
    if let Some(term) = &fragment.term {
        visitor.visit_term(term);
    }
}

pub fn walk_stmt<V, P>(visitor: &mut V, stmt: &StructuredPassStmt<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
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
    PassExpr<P>: MapExpr<PassExpr<P>>,
{
    let _ = visitor;
    let _ = label;
}

pub fn walk_term<V, P>(visitor: &mut V, term: &BlockPyTerm<PassExpr<P>>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
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

pub fn walk_expr<V, P>(visitor: &mut V, expr: &PassExpr<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
{
    let _ = expr.clone().map_expr(&mut |child| {
        visitor.visit_expr(&child);
        child
    });
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

impl ImplicitNoneExpr for CoreBlockPyExprWithAwaitAndYield {
    fn implicit_none_expr() -> Self {
        Self::Name(implicit_none_name())
    }

    fn is_implicit_none_expr(expr: &Self) -> bool {
        matches!(expr, CoreBlockPyExprWithAwaitAndYield::Name(name) if name.id.as_str() == "__dp_NONE")
    }
}

impl ImplicitNoneExpr for CoreBlockPyExprWithYield {
    fn implicit_none_expr() -> Self {
        Self::Name(implicit_none_name())
    }

    fn is_implicit_none_expr(expr: &Self) -> bool {
        matches!(expr, CoreBlockPyExprWithYield::Name(name) if name.id.as_str() == "__dp_NONE")
    }
}

impl<N: BlockPyNameLike> ImplicitNoneExpr for CoreBlockPyExpr<N> {
    fn implicit_none_expr() -> Self {
        Self::Name(implicit_none_name().into())
    }

    fn is_implicit_none_expr(expr: &Self) -> bool {
        matches!(expr, CoreBlockPyExpr::Name(name) if name.id_str() == "__dp_NONE")
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
}

#[cfg(test)]
mod test;

impl BlockPyLabel {
    pub fn as_str(&self) -> &str {
        self.0.as_str()
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
