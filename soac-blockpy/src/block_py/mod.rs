pub use self::meta::{HasMeta, Meta, WithMeta};
use self::operation_macro::define_operation;
pub use self::param_specs::{Param, ParamDefaultSource, ParamKind, ParamSpec};
pub(crate) use self::semantics::{
    build_storage_layout_from_capture_names, compute_make_function_capture_bindings_from_semantics,
    compute_storage_layout_from_semantics, derive_effective_binding_for_name,
    BlockPySemanticExprNode,
};
pub use self::semantics::{
    BindingTarget, BlockPyBindingKind, BlockPyBindingPurpose, BlockPyCallableScopeKind,
    BlockPyCallableSemanticInfo, BlockPyCellBindingKind, BlockPyCellCaptureBinding,
    BlockPyClassBodyFallback, BlockPyEffectiveBinding, ClosureInit, ClosureSlot, StorageLayout,
};
use crate::passes::{CodegenBlockPyPass, ResolvedStorageBlockPyPass};
use crate::py_expr;
pub use operation::{
    BinOp, BinOpKind, Call, CellRef, CellRefForName, Del, DelItem, GetAttr, GetItem, Load,
    MakeCell, MakeFunction, SetAttr, SetItem, Store, UnaryOp, UnaryOpKind,
};
pub use ruff_python_ast::Expr;
use ruff_python_ast::{self as ast, ExprName};
use soac_macros::{with_match_default, DelegateMatchDefault};
use std::fmt;

pub(crate) mod cfg;
mod convert;
pub(crate) mod dataflow;
pub(crate) mod exception;
mod meta;
mod name_gen;
pub mod operation;
mod operation_macro;
pub(crate) mod param_specs;
pub mod pretty;
pub(crate) mod semantics;
pub(crate) mod structured;
pub(crate) mod validate;
#[cfg(test)]
pub(crate) use convert::BlockPyModuleTryMap;
pub(crate) use convert::{
    try_lower_core_expr_without_await, try_lower_core_expr_without_yield, BlockPyModuleMap,
    ExprTryMap,
};
pub use name_gen::{BlockPyLabel, FunctionNameGen, ModuleNameGen};
pub(crate) use validate::validate_module;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FunctionId(pub usize);

fn is_internal_symbol(name: &str) -> bool {
    name.starts_with("_dp_") || name == "__soac__"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalLocation(pub u32);

impl LocalLocation {
    pub fn slot(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellLocation {
    Owned(u32),
    Closure(u32),
    CapturedSource(u32),
}

impl CellLocation {
    pub fn slot(self) -> u32 {
        match self {
            Self::Owned(slot) | Self::Closure(slot) | Self::CapturedSource(slot) => slot,
        }
    }

    pub fn is_owned(self) -> bool {
        matches!(self, Self::Owned(_))
    }

    pub fn is_closure(self) -> bool {
        matches!(self, Self::Closure(_))
    }

    pub fn is_captured_source(self) -> bool {
        matches!(self, Self::CapturedSource(_))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NameLocation {
    Local(LocalLocation),
    Global,
    RuntimeName,
    Cell(CellLocation),
    Constant(u32),
}

impl NameLocation {
    pub fn local(slot: u32) -> Self {
        Self::Local(LocalLocation(slot))
    }

    pub fn global() -> Self {
        Self::Global
    }

    pub fn runtime_name() -> Self {
        Self::RuntimeName
    }

    pub fn owned_cell(slot: u32) -> Self {
        Self::Cell(CellLocation::Owned(slot))
    }

    pub fn closure_cell(slot: u32) -> Self {
        Self::Cell(CellLocation::Closure(slot))
    }

    pub fn captured_source_cell(slot: u32) -> Self {
        Self::Cell(CellLocation::CapturedSource(slot))
    }

    pub fn constant(index: u32) -> Self {
        Self::Constant(index)
    }

    pub fn as_local(self) -> Option<LocalLocation> {
        match self {
            Self::Local(location) => Some(location),
            Self::Global | Self::RuntimeName | Self::Cell(_) | Self::Constant(_) => None,
        }
    }

    pub fn as_cell(self) -> Option<CellLocation> {
        match self {
            Self::Cell(location) => Some(location),
            Self::Local(_) | Self::Global | Self::RuntimeName | Self::Constant(_) => None,
        }
    }

    pub fn as_constant(self) -> Option<u32> {
        match self {
            Self::Constant(index) => Some(index),
            Self::Local(_) | Self::Global | Self::RuntimeName | Self::Cell(_) => None,
        }
    }

    pub fn is_global(self) -> bool {
        matches!(self, Self::Global)
    }

    pub fn is_runtime_name(self) -> bool {
        matches!(self, Self::RuntimeName)
    }

    pub fn pretty_id(self, unresolved_name: &str) -> String {
        match self {
            Self::Local(location) => format!("{location:?}"),
            Self::Global => unresolved_name.to_string(),
            Self::RuntimeName => unresolved_name.to_string(),
            Self::Cell(location) => format!("{location:?}"),
            Self::Constant(index) => format!("constant slot {index}"),
        }
    }
}

pub trait BlockPyNameLike: Clone + fmt::Debug + From<ast::ExprName> {
    fn id_str(&self) -> &str;
    fn pretty_id(&self) -> String {
        self.id_str().to_string()
    }
    fn is_runtime_name(&self) -> bool {
        false
    }
    fn is_runtime_symbol(&self, name: &str) -> bool {
        self.is_runtime_name() && self.id_str() == name
    }
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

pub trait MapExpr<T>: Clone + fmt::Debug + Sized {
    fn map_expr(self, f: &mut impl FnMut(Self) -> T) -> T;
}

pub trait TryMapExpr<T, Error>: Clone + fmt::Debug + Sized {
    fn try_map_expr(self, f: &mut impl FnMut(Self) -> Result<T, Error>) -> Result<T, Error>;
}

pub trait Instr: Clone + fmt::Debug + Sized {
    type Name: BlockPyNameLike;
}

pub trait InstrExprNode<I>: Sized + HasMeta + WithMeta
where
    I: Instr,
{
    type Mapped<T: Instr>;

    fn visit_exprs(&self, f: &mut impl FnMut(&I));
    fn visit_exprs_mut(&mut self, f: &mut impl FnMut(&mut I));
    fn map_expr_node<T>(self, f: &mut impl FnMut(I) -> T) -> Self::Mapped<T>
    where
        T: Instr,
        InstrName<T>: From<InstrName<I>>;
    fn try_map_expr_node<T, Error>(
        self,
        f: &mut impl FnMut(I) -> Result<T, Error>,
    ) -> Result<Self::Mapped<T>, Error>
    where
        T: Instr,
        InstrName<T>: From<InstrName<I>>;
}

pub trait BlockPyExprLike: Clone + fmt::Debug + MapExpr<Self> {
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

impl Instr for Expr {
    type Name = ast::ExprName;
}

#[derive(Clone)]
pub enum UnresolvedName {
    ExprName(ast::ExprName),
    RuntimeName(CoreStringLiteral),
}

impl fmt::Debug for UnresolvedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExprName(name) => f
                .debug_struct("ExprName")
                .field("id", &name.id)
                .field("ctx", &name.ctx)
                .finish(),
            Self::RuntimeName(name) => f.debug_tuple("RuntimeName").field(name).finish(),
        }
    }
}

impl BlockPyNameLike for UnresolvedName {
    fn id_str(&self) -> &str {
        match self {
            Self::ExprName(name) => name.id.as_str(),
            Self::RuntimeName(name) => name.value.as_str(),
        }
    }

    fn is_runtime_name(&self) -> bool {
        matches!(self, Self::RuntimeName(_))
    }
}

impl From<ast::ExprName> for UnresolvedName {
    fn from(value: ast::ExprName) -> Self {
        Self::ExprName(value)
    }
}

impl UnresolvedName {
    pub fn into_expr_name(self) -> ast::ExprName {
        match self {
            Self::ExprName(name) => name,
            Self::RuntimeName(name) => ast::ExprName {
                id: name.value.into(),
                ctx: ast::ExprContext::Load,
                range: name.range,
                node_index: name.node_index,
            },
        }
    }
}

impl MapExpr<RuffExpr> for RuffExpr {
    fn map_expr(self, f: &mut impl FnMut(Self) -> RuffExpr) -> RuffExpr {
        RuffExpr(self.0.map_expr(&mut |expr| f(RuffExpr(expr)).0))
    }
}

impl<T> BlockPyExprLike for T where T: Clone + fmt::Debug + MapExpr<Self> {}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LocatedName {
    pub id: ruff_python_ast::name::Name,
    pub location: NameLocation,
}

impl LocatedName {
    pub fn with_location(mut self, location: NameLocation) -> Self {
        self.location = location;
        self
    }

    pub fn local_location(&self) -> Option<LocalLocation> {
        self.location.as_local()
    }

    pub fn cell_location(&self) -> Option<CellLocation> {
        self.location.as_cell()
    }

    pub fn resolved_pretty_id(&self) -> String {
        self.location.pretty_id(self.id.as_str())
    }

    pub fn is_runtime_name(&self) -> bool {
        self.location.is_runtime_name()
    }
}

impl BlockPyNameLike for LocatedName {
    fn id_str(&self) -> &str {
        self.id.as_str()
    }

    fn pretty_id(&self) -> String {
        self.resolved_pretty_id()
    }

    fn is_runtime_name(&self) -> bool {
        self.location.is_runtime_name()
    }
}

impl From<ast::ExprName> for LocatedName {
    fn from(value: ast::ExprName) -> Self {
        Self {
            id: value.id,
            location: NameLocation::Global,
        }
    }
}

impl From<UnresolvedName> for LocatedName {
    fn from(value: UnresolvedName) -> Self {
        match value {
            UnresolvedName::ExprName(name) => Self::from(name),
            UnresolvedName::RuntimeName(name) => Self {
                id: name.value.into(),
                location: NameLocation::RuntimeName,
            },
        }
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
pub struct CfgBlock<S, T = BlockPyTerm<S>> {
    pub label: BlockPyLabel,
    pub body: Vec<S>,
    pub term: T,
    pub params: Vec<BlockParam>,
    pub exc_edge: Option<BlockPyEdge>,
}

impl<S, T> CfgBlock<S, T> {
    pub fn label_str(&self) -> String {
        self.label.to_string()
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
        self.params
            .retain(|param| param.role != BlockParamRole::Exception || param.name == name);
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
pub struct BlockPyModule<P: BlockPyPass, S = <P as BlockPyPass>::Expr> {
    pub callable_defs: Vec<BlockPyFunction<P, S>>,
    pub module_constants: Vec<CoreBlockPyExpr<LocatedName>>,
}

impl<P: BlockPyPass, S> BlockPyModule<P, S> {
    pub fn map_callable_defs<Q: BlockPyPass, T>(
        self,
        mut f: impl FnMut(BlockPyFunction<P, S>) -> BlockPyFunction<Q, T>,
    ) -> BlockPyModule<Q, T> {
        debug_assert!(
            self.module_constants.is_empty(),
            "map_callable_defs does not preserve module constants"
        );
        BlockPyModule {
            callable_defs: self.callable_defs.into_iter().map(&mut f).collect(),
            module_constants: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, derive_more::From, DelegateMatchDefault)]
pub enum CoreBlockPyExprWithAwaitAndYield {
    Literal(LiteralValue),
    BinOp(BinOp<Self>),
    UnaryOp(UnaryOp<Self>),
    Call(Call<Self>),
    GetAttr(GetAttr<Self>),
    SetAttr(SetAttr<Self>),
    GetItem(GetItem<Self>),
    SetItem(SetItem<Self>),
    DelItem(DelItem<Self>),
    Load(Load<Self>),
    Store(Store<Self>),
    Del(Del<Self>),
    MakeCell(MakeCell<Self>),
    CellRefForName(CellRefForName),
    CellRef(CellRef),
    MakeFunction(MakeFunction<Self>),
    Await(CoreBlockPyAwait<Self>),
    Yield(CoreBlockPyYield<Self>),
    YieldFrom(CoreBlockPyYieldFrom<Self>),
}

#[derive(Debug, Clone, derive_more::From, DelegateMatchDefault)]
pub enum CoreBlockPyExprWithYield {
    Literal(LiteralValue),
    BinOp(BinOp<Self>),
    UnaryOp(UnaryOp<Self>),
    Call(Call<Self>),
    GetAttr(GetAttr<Self>),
    SetAttr(SetAttr<Self>),
    GetItem(GetItem<Self>),
    SetItem(SetItem<Self>),
    DelItem(DelItem<Self>),
    Load(Load<Self>),
    Store(Store<Self>),
    Del(Del<Self>),
    MakeCell(MakeCell<Self>),
    CellRefForName(CellRefForName),
    CellRef(CellRef),
    MakeFunction(MakeFunction<Self>),
    Yield(CoreBlockPyYield<Self>),
    YieldFrom(CoreBlockPyYieldFrom<Self>),
}

#[derive(Debug, Clone, derive_more::From, DelegateMatchDefault)]
pub enum CoreBlockPyExpr<N: BlockPyNameLike = UnresolvedName> {
    Literal(LiteralValue),
    BinOp(BinOp<Self>),
    UnaryOp(UnaryOp<Self>),
    Call(Call<Self>),
    GetAttr(GetAttr<Self>),
    SetAttr(SetAttr<Self>),
    GetItem(GetItem<Self>),
    SetItem(SetItem<Self>),
    DelItem(DelItem<Self>),
    Load(Load<Self>),
    Store(Store<Self>),
    Del(Del<Self>),
    MakeCell(MakeCell<Self>),
    CellRefForName(CellRefForName),
    CellRef(CellRef),
    MakeFunction(MakeFunction<Self>),
}

pub type LocatedCoreBlockPyExpr = CoreBlockPyExpr<LocatedName>;

#[derive(Debug, Clone, derive_more::From, DelegateMatchDefault)]
pub enum CodegenBlockPyExpr {
    Literal(LiteralValue),
    BinOp(BinOp<Self>),
    UnaryOp(UnaryOp<Self>),
    Call(Call<Self>),
    GetAttr(GetAttr<Self>),
    SetAttr(SetAttr<Self>),
    GetItem(GetItem<Self>),
    SetItem(SetItem<Self>),
    DelItem(DelItem<Self>),
    Load(Load<Self>),
    Store(Store<Self>),
    Del(Del<Self>),
    MakeCell(MakeCell<Self>),
    CellRefForName(CellRefForName),
    CellRef(CellRef),
    MakeFunction(MakeFunction<Self>),
}

pub type LocatedCodegenBlockPyExpr = CodegenBlockPyExpr;

#[derive(Debug, Clone)]
pub enum BlockPyLiteral {
    StringLiteral(CoreStringLiteral),
    BytesLiteral(CoreBytesLiteral),
    NumberLiteral(CoreNumberLiteral),
}

define_operation! {
    pub struct LiteralValue {
        literal: BlockPyLiteral,
    }
}

impl LiteralValue {
    pub fn as_literal(&self) -> &BlockPyLiteral {
        &self.literal
    }

    pub fn into_literal(self) -> BlockPyLiteral {
        self.literal
    }
}

impl From<BlockPyLiteral> for LiteralValue {
    fn from(literal: BlockPyLiteral) -> Self {
        let meta = literal.meta();
        LiteralValue::new(literal).with_meta(meta)
    }
}

impl From<BlockPyLiteral> for CoreBlockPyExprWithAwaitAndYield {
    fn from(literal: BlockPyLiteral) -> Self {
        Self::Literal(literal.into())
    }
}

impl From<BlockPyLiteral> for CoreBlockPyExprWithYield {
    fn from(literal: BlockPyLiteral) -> Self {
        Self::Literal(literal.into())
    }
}

impl<N: BlockPyNameLike> From<BlockPyLiteral> for CoreBlockPyExpr<N> {
    fn from(literal: BlockPyLiteral) -> Self {
        Self::Literal(literal.into())
    }
}

impl From<BlockPyLiteral> for CodegenBlockPyExpr {
    fn from(literal: BlockPyLiteral) -> Self {
        Self::Literal(literal.into())
    }
}

pub type CoreBlockPyLiteral = BlockPyLiteral;
pub type CodegenBlockPyLiteral = BlockPyLiteral;
impl<I: Instr<Name = UnresolvedName>> InstrExprNode<I> for UnresolvedName {
    type Mapped<T: Instr> = InstrName<T>;

    fn visit_exprs(&self, _f: &mut impl FnMut(&I)) {}

    fn visit_exprs_mut(&mut self, _f: &mut impl FnMut(&mut I)) {}

    fn map_expr_node<T>(self, _f: &mut impl FnMut(I) -> T) -> Self::Mapped<T>
    where
        T: Instr,
        InstrName<T>: From<InstrName<I>>,
    {
        <T as Instr>::Name::from(self)
    }

    fn try_map_expr_node<T, Error>(
        self,
        _f: &mut impl FnMut(I) -> Result<T, Error>,
    ) -> Result<Self::Mapped<T>, Error>
    where
        T: Instr,
        InstrName<T>: From<InstrName<I>>,
    {
        Ok(<T as Instr>::Name::from(self))
    }
}

impl<I: Instr> InstrExprNode<I> for BlockPyLiteral {
    type Mapped<T: Instr> = BlockPyLiteral;

    fn visit_exprs(&self, _f: &mut impl FnMut(&I)) {}

    fn visit_exprs_mut(&mut self, _f: &mut impl FnMut(&mut I)) {}

    fn map_expr_node<T>(self, _f: &mut impl FnMut(I) -> T) -> Self::Mapped<T>
    where
        T: Instr,
        InstrName<T>: From<InstrName<I>>,
    {
        self
    }

    fn try_map_expr_node<T, Error>(
        self,
        _f: &mut impl FnMut(I) -> Result<T, Error>,
    ) -> Result<Self::Mapped<T>, Error>
    where
        T: Instr,
        InstrName<T>: From<InstrName<I>>,
    {
        Ok(self)
    }
}

#[derive(Clone)]
pub struct CoreStringLiteral {
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub value: String,
}

impl fmt::Debug for CoreStringLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoreStringLiteral")
            .field("value", &self.value)
            .finish()
    }
}

#[derive(Clone)]
pub struct CoreBytesLiteral {
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub value: Vec<u8>,
}

impl fmt::Debug for CoreBytesLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoreBytesLiteral")
            .field("value", &self.value)
            .finish()
    }
}

#[derive(Clone)]
pub struct CoreNumberLiteral {
    pub node_index: ast::AtomicNodeIndex,
    pub range: ruff_text_size::TextRange,
    pub value: CoreNumberLiteralValue,
}

impl fmt::Debug for CoreNumberLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CoreNumberLiteral")
            .field("value", &self.value)
            .finish()
    }
}

#[derive(Debug, Clone)]
pub enum CoreNumberLiteralValue {
    Int(ast::Int),
    Float(f64),
}

impl Instr for CoreBlockPyExprWithAwaitAndYield {
    type Name = UnresolvedName;
}

#[with_match_default]
impl HasMeta for CoreBlockPyExprWithAwaitAndYield {
    fn meta(&self) -> Meta {
        match self {
            match_rest(node) => node.meta(),
        }
    }
}

#[with_match_default]
impl WithMeta for CoreBlockPyExprWithAwaitAndYield {
    fn with_meta(self, meta: Meta) -> Self {
        match self {
            match_rest(node) => node.with_meta(meta.clone()).into(),
        }
    }
}

#[with_match_default]
impl MapExpr<CoreBlockPyExprWithAwaitAndYield> for CoreBlockPyExprWithAwaitAndYield {
    fn map_expr(
        self,
        f: &mut impl FnMut(Self) -> CoreBlockPyExprWithAwaitAndYield,
    ) -> CoreBlockPyExprWithAwaitAndYield {
        match self {
            match_rest(node) => node.map_expr_node(&mut *f).into(),
        }
    }
}

#[with_match_default]
impl MapExpr<CoreBlockPyExprWithYield> for CoreBlockPyExprWithAwaitAndYield {
    fn map_expr(
        self,
        f: &mut impl FnMut(Self) -> CoreBlockPyExprWithYield,
    ) -> CoreBlockPyExprWithYield {
        match self {
            Self::Await(await_expr) => {
                let meta = await_expr.meta();
                CoreBlockPyExprWithYield::YieldFrom(
                    CoreBlockPyYieldFrom::new(core_runtime_positional_call_expr_with_meta(
                        "await_iter",
                        meta.node_index.clone(),
                        meta.range,
                        vec![f(*await_expr.value)],
                    ))
                    .with_meta(meta),
                )
            }
            match_rest(node) => node.map_expr_node(&mut *f).into(),
        }
    }
}

#[with_match_default]
impl TryMapExpr<CoreBlockPyExprWithYield, CoreBlockPyExprWithAwaitAndYield>
    for CoreBlockPyExprWithAwaitAndYield
{
    fn try_map_expr(
        self,
        f: &mut impl FnMut(Self) -> Result<CoreBlockPyExprWithYield, CoreBlockPyExprWithAwaitAndYield>,
    ) -> Result<CoreBlockPyExprWithYield, CoreBlockPyExprWithAwaitAndYield> {
        match self {
            Self::Await(_) => Err(self),
            match_rest(node) => Ok(node.try_map_expr_node(&mut *f)?.into()),
        }
    }
}

impl Instr for CoreBlockPyExprWithYield {
    type Name = UnresolvedName;
}

#[with_match_default]
impl HasMeta for CoreBlockPyExprWithYield {
    fn meta(&self) -> Meta {
        match self {
            match_rest(node) => node.meta(),
        }
    }
}

#[with_match_default]
impl WithMeta for CoreBlockPyExprWithYield {
    fn with_meta(self, meta: Meta) -> Self {
        match self {
            match_rest(node) => node.with_meta(meta.clone()).into(),
        }
    }
}

#[with_match_default]
impl MapExpr<CoreBlockPyExprWithYield> for CoreBlockPyExprWithYield {
    fn map_expr(
        self,
        f: &mut impl FnMut(Self) -> CoreBlockPyExprWithYield,
    ) -> CoreBlockPyExprWithYield {
        match self {
            match_rest(node) => node.map_expr_node(&mut *f).into(),
        }
    }
}

#[with_match_default]
impl TryMapExpr<CoreBlockPyExpr, CoreBlockPyExprWithYield> for CoreBlockPyExprWithYield {
    fn try_map_expr(
        self,
        f: &mut impl FnMut(Self) -> Result<CoreBlockPyExpr, CoreBlockPyExprWithYield>,
    ) -> Result<CoreBlockPyExpr, CoreBlockPyExprWithYield> {
        match self {
            Self::Yield(_) => Err(self),
            Self::YieldFrom(_) => Err(self),
            match_rest(node) => Ok(node.try_map_expr_node(&mut *f)?.into()),
        }
    }
}

impl<N: BlockPyNameLike> Instr for CoreBlockPyExpr<N> {
    type Name = N;
}

#[with_match_default]
impl<N: BlockPyNameLike> HasMeta for CoreBlockPyExpr<N> {
    fn meta(&self) -> Meta {
        match self {
            match_rest(node) => node.meta(),
        }
    }
}

#[with_match_default]
impl<N: BlockPyNameLike> WithMeta for CoreBlockPyExpr<N> {
    fn with_meta(self, meta: Meta) -> Self {
        match self {
            match_rest(node) => node.with_meta(meta.clone()).into(),
        }
    }
}

#[with_match_default]
impl<NIn, NOut> MapExpr<CoreBlockPyExpr<NOut>> for CoreBlockPyExpr<NIn>
where
    NIn: BlockPyNameLike,
    NOut: BlockPyNameLike + From<NIn>,
{
    fn map_expr(self, f: &mut impl FnMut(Self) -> CoreBlockPyExpr<NOut>) -> CoreBlockPyExpr<NOut> {
        match self {
            match_rest(node) => node.map_expr_node(&mut *f).into(),
        }
    }
}

#[with_match_default]
impl<NIn, NOut, Error> TryMapExpr<CoreBlockPyExpr<NOut>, Error> for CoreBlockPyExpr<NIn>
where
    NIn: BlockPyNameLike,
    NOut: BlockPyNameLike + From<NIn>,
{
    fn try_map_expr(
        self,
        f: &mut impl FnMut(Self) -> Result<CoreBlockPyExpr<NOut>, Error>,
    ) -> Result<CoreBlockPyExpr<NOut>, Error> {
        match self {
            match_rest(node) => Ok(node.try_map_expr_node(&mut *f)?.into()),
        }
    }
}

impl Instr for CodegenBlockPyExpr {
    type Name = LocatedName;
}

#[with_match_default]
impl HasMeta for CodegenBlockPyExpr {
    fn meta(&self) -> Meta {
        match self {
            match_rest(op) => op.meta(),
        }
    }
}

#[with_match_default]
impl WithMeta for CodegenBlockPyExpr {
    fn with_meta(self, meta: Meta) -> Self {
        match self {
            match_rest(op) => op.with_meta(meta.clone()).into(),
        }
    }
}

#[with_match_default]
impl<NIn> MapExpr<CodegenBlockPyExpr> for CoreBlockPyExpr<NIn>
where
    NIn: BlockPyNameLike,
    LocatedName: From<NIn>,
{
    fn map_expr(self, f: &mut impl FnMut(Self) -> CodegenBlockPyExpr) -> CodegenBlockPyExpr {
        match self {
            Self::Literal(_) => {
                panic!("core literals should normalize into Load(Constant(_)) before codegen")
            }
            match_rest(node) => node.map_expr_node(&mut *f).into(),
        }
    }
}

#[with_match_default]
impl<Error> TryMapExpr<CodegenBlockPyExpr, Error> for CodegenBlockPyExpr {
    fn try_map_expr(
        self,
        f: &mut impl FnMut(Self) -> Result<CodegenBlockPyExpr, Error>,
    ) -> Result<CodegenBlockPyExpr, Error> {
        Ok(match self {
            match_rest(op) => op.try_map_expr_node(&mut *f)?.into(),
        })
    }
}

#[with_match_default]
impl MapExpr<CodegenBlockPyExpr> for CodegenBlockPyExpr {
    fn map_expr(self, f: &mut impl FnMut(Self) -> CodegenBlockPyExpr) -> CodegenBlockPyExpr {
        match self {
            match_rest(op) => op.map_expr_node(&mut *f).into(),
        }
    }
}

pub(crate) fn core_call_expr_with_meta<E>(
    func: E,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<CoreBlockPyCallArg<E>>,
    keywords: Vec<CoreBlockPyKeywordArg<E>>,
) -> E
where
    E: Instr + From<Call<E>>,
{
    Call::new(func, args, keywords)
        .with_meta(Meta::new(node_index, range))
        .into()
}

pub(crate) fn core_named_call_expr_with_meta<E>(
    func_name: &str,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<CoreBlockPyCallArg<E>>,
    keywords: Vec<CoreBlockPyKeywordArg<E>>,
) -> E
where
    E: Instr<Name = UnresolvedName> + From<Call<E>> + From<Load<E>>,
{
    let func = Load::new(UnresolvedName::from(ExprName {
        id: func_name.into(),
        ctx: ast::ExprContext::Load,
        range,
        node_index: node_index.clone(),
    }))
    .with_meta(Meta::new(node_index.clone(), range))
    .into();
    core_call_expr_with_meta(func, node_index, range, args, keywords)
}

pub(crate) fn core_runtime_name_expr_with_meta<E>(
    name: &str,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> E
where
    E: Instr<Name = UnresolvedName> + From<Load<E>>,
{
    core_operation_expr(
        Load::new(runtime_symbol(name, node_index.clone(), range))
            .with_meta(Meta::new(node_index, range)),
    )
}

pub(crate) fn core_operation_expr<E>(operation: impl Into<E>) -> E {
    operation.into()
}

pub(crate) fn core_positional_call_expr_with_meta<E>(
    func_name: &str,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<E>,
) -> E
where
    E: Instr<Name = UnresolvedName> + From<Call<E>> + From<Load<E>>,
{
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

pub(crate) fn core_runtime_named_call_expr_with_meta<E>(
    func_name: &str,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<CoreBlockPyCallArg<E>>,
    keywords: Vec<CoreBlockPyKeywordArg<E>>,
) -> E
where
    E: Instr<Name = UnresolvedName> + From<Call<E>> + From<Load<E>>,
{
    let func = core_runtime_name_expr_with_meta(func_name, node_index.clone(), range);
    core_call_expr_with_meta(func, node_index, range, args, keywords)
}

pub(crate) fn core_runtime_positional_call_expr_with_meta<E>(
    func_name: &str,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
    args: Vec<E>,
) -> E
where
    E: Instr<Name = UnresolvedName> + From<Call<E>> + From<Load<E>>,
{
    core_runtime_named_call_expr_with_meta(
        func_name,
        node_index,
        range,
        args.into_iter()
            .map(CoreBlockPyCallArg::Positional)
            .collect(),
        Vec::new(),
    )
}

pub(crate) fn runtime_symbol(
    name: &str,
    node_index: ast::AtomicNodeIndex,
    range: ruff_text_size::TextRange,
) -> UnresolvedName {
    UnresolvedName::RuntimeName(CoreStringLiteral {
        node_index,
        range,
        value: name.to_string(),
    })
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

define_operation! {
    pub struct CoreBlockPyAwait<E> {
        value: Box<E>,
    }
}

define_operation! {
    pub struct CoreBlockPyYield<E> {
        value: Box<E>,
    }
}

define_operation! {
    pub struct CoreBlockPyYieldFrom<E> {
        value: Box<E>,
    }
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

#[derive(Debug)]
pub struct BlockPyFunction<P: BlockPyPass, S = <P as BlockPyPass>::Expr> {
    pub function_id: FunctionId,
    pub name_gen: FunctionNameGen,
    pub names: FunctionName,
    pub kind: BlockPyFunctionKind,
    pub params: ParamSpec,
    pub blocks: Vec<CfgBlock<S, BlockPyTerm<P::Expr>>>,
    pub doc: Option<String>,
    pub storage_layout: Option<StorageLayout>,
    pub semantic: BlockPyCallableSemanticInfo,
}

impl<P: BlockPyPass, S: Clone> Clone for BlockPyFunction<P, S> {
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
            storage_layout: self.storage_layout.clone(),
            semantic: self.semantic.clone(),
        }
    }
}

impl<P: BlockPyPass, S> BlockPyFunction<P, S> {
    pub fn lowered_kind(&self) -> &BlockPyFunctionKind {
        &self.kind
    }

    pub fn storage_layout(&self) -> &Option<StorageLayout> {
        &self.storage_layout
    }

    pub fn entry_block(&self) -> &CfgBlock<S, BlockPyTerm<P::Expr>> {
        self.blocks
            .first()
            .expect("BlockPyFunction should have at least one block")
    }

    pub fn map_blocks<Q: BlockPyPass, T>(
        self,
        mut f: impl FnMut(CfgBlock<S, BlockPyTerm<P::Expr>>) -> CfgBlock<T, BlockPyTerm<Q::Expr>>,
    ) -> BlockPyFunction<Q, T> {
        BlockPyFunction {
            function_id: self.function_id,
            name_gen: self.name_gen,
            names: self.names,
            kind: self.kind,
            params: self.params,
            blocks: self.blocks.into_iter().map(&mut f).collect(),
            doc: self.doc,
            storage_layout: self.storage_layout,
            semantic: self.semantic,
        }
    }
}

pub trait BlockPyNormalizedStmt {
    fn assert_blockpy_normalized(&self);
}

pub trait BlockPyPass: Clone + fmt::Debug {
    type Expr: BlockPyExprLike + Instr;
}

pub type InstrName<I> = <I as Instr>::Name;
pub type ResolvedStorageBlock = CfgBlock<LocatedCoreBlockPyExpr>;
pub type CodegenBlock = CfgBlock<CodegenBlockPyExpr>;

pub(crate) type BlockPyBlock<E = Expr> = CfgBlock<StructuredInstr<E>, BlockPyTerm<E>>;

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

pub(crate) type BlockPyStmtFragment<E = Expr> =
    BlockPyCfgFragment<StructuredInstr<E>, BlockPyTerm<E>>;

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

pub(crate) type BlockPyStmtFragmentBuilder<E = Expr> =
    BlockPyCfgFragmentBuilder<StructuredInstr<E>, BlockPyTerm<E>>;

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
            "cannot append structured BlockPy stmt after stmt-fragment terminator"
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

pub(crate) type BlockPyBlockBuilder<E = Expr> =
    BlockPyCfgBlockBuilder<StructuredInstr<E>, BlockPyTerm<E>>;

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
            self.params
                .retain(|param| param.role != BlockParamRole::Exception || param.name == exc_param);
            if self.params.iter().any(|param| param.name == exc_param) {
                for param in &mut self.params {
                    if param.name == exc_param {
                        param.role = BlockParamRole::Exception;
                    }
                }
            } else {
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

    pub fn finish(self, fallthrough_target: Option<BlockPyLabel>) -> CfgBlock<S, T> {
        let fragment = self.fragment.finish();
        let block = CfgBlock {
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
pub(crate) enum StructuredInstr<E = Expr> {
    Expr(E),
    If(StructuredIf<E>),
}

impl<I> From<Del<I>> for StructuredInstr<I>
where
    I: Instr + From<Del<I>>,
{
    fn from(value: Del<I>) -> Self {
        Self::Expr(value.into())
    }
}

impl<E: std::fmt::Debug> StructuredInstr<E> {
    pub fn assert_normalized(&self) {
        if let Self::If(if_stmt) = self {
            if_stmt.body.assert_normalized();
            if_stmt.orelse.assert_normalized();
        }
    }
}

impl<E: std::fmt::Debug> BlockPyNormalizedStmt for StructuredInstr<E> {
    fn assert_blockpy_normalized(&self) {
        self.assert_normalized();
    }
}

impl<I> BlockPyNormalizedStmt for I
where
    I: Instr,
{
    fn assert_blockpy_normalized(&self) {}
}

pub(crate) fn convert_structured_instr_expr<EIn, EOut>(
    value: StructuredInstr<EIn>,
) -> StructuredInstr<EOut>
where
    EOut: From<EIn>,
{
    match value {
        StructuredInstr::Expr(expr) => StructuredInstr::Expr(expr.into()),
        StructuredInstr::If(if_stmt) => StructuredInstr::If(StructuredIf {
            test: if_stmt.test.into(),
            body: convert_blockpy_fragment_expr(if_stmt.body),
            orelse: convert_blockpy_fragment_expr(if_stmt.orelse),
        }),
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
pub struct BlockPyAssign<E = Expr, N = InstrName<E>> {
    pub target: N,
    pub value: E,
}

#[derive(Debug, Clone)]
pub struct BlockPyDelete<N = ExprName> {
    pub target: N,
}

#[derive(Debug, Clone)]
pub(crate) struct StructuredIf<E = Expr> {
    pub test: E,
    pub body: BlockPyCfgFragment<StructuredInstr<E>, BlockPyTerm<E>>,
    pub orelse: BlockPyCfgFragment<StructuredInstr<E>, BlockPyTerm<E>>,
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

pub(crate) fn convert_blockpy_fragment_expr<EIn, EOut>(
    value: BlockPyCfgFragment<StructuredInstr<EIn>, BlockPyTerm<EIn>>,
) -> BlockPyCfgFragment<StructuredInstr<EOut>, BlockPyTerm<EOut>>
where
    EOut: From<EIn>,
{
    BlockPyCfgFragment {
        body: value
            .body
            .into_iter()
            .map(convert_structured_instr_expr)
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
    Exception,
    AbruptKind,
    AbruptPayload,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BlockParam {
    pub name: String,
    pub role: BlockParamRole,
}

pub(crate) trait BlockPyLinearModuleVisitor<P>
where
    P: BlockPyPass,
    P::Expr: MapExpr<P::Expr>,
{
    fn visit_module(&mut self, module: &BlockPyModule<P>) {
        walk_linear_module(self, module);
    }

    fn visit_fn(&mut self, func: &BlockPyFunction<P>) {
        walk_linear_fn(self, func);
    }

    fn visit_block(&mut self, block: &CfgBlock<P::Expr>) {
        walk_linear_block(self, block);
    }

    fn visit_stmt(&mut self, stmt: &P::Expr) {
        walk_linear_stmt(self, stmt);
    }

    fn visit_term(&mut self, term: &BlockPyTerm<P::Expr>) {
        walk_linear_term(self, term);
    }

    fn visit_label(&mut self, label: &BlockPyLabel) {
        walk_linear_label::<Self, P>(self, label);
    }

    fn visit_expr(&mut self, expr: &P::Expr) {
        walk_linear_expr(self, expr);
    }
}

pub(crate) fn walk_linear_module<V, P>(visitor: &mut V, module: &BlockPyModule<P>)
where
    V: BlockPyLinearModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: MapExpr<P::Expr>,
{
    for function in &module.callable_defs {
        visitor.visit_fn(function);
    }
}

pub(crate) fn walk_linear_fn<V, P>(visitor: &mut V, func: &BlockPyFunction<P>)
where
    V: BlockPyLinearModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: MapExpr<P::Expr>,
{
    for block in &func.blocks {
        visitor.visit_block(block);
    }
}

pub(crate) fn walk_linear_block<V, P>(visitor: &mut V, block: &CfgBlock<P::Expr>)
where
    V: BlockPyLinearModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: MapExpr<P::Expr>,
{
    for stmt in &block.body {
        visitor.visit_stmt(stmt);
    }
    if let Some(exc_edge) = &block.exc_edge {
        visitor.visit_label(&exc_edge.target);
    }
    visitor.visit_term(&block.term);
}

pub(crate) fn walk_linear_stmt<V, P>(visitor: &mut V, stmt: &P::Expr)
where
    V: BlockPyLinearModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: MapExpr<P::Expr>,
{
    visitor.visit_expr(stmt);
}

pub(crate) fn walk_linear_label<V, P>(visitor: &mut V, label: &BlockPyLabel)
where
    V: BlockPyLinearModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: MapExpr<P::Expr>,
{
    let _ = visitor;
    let _ = label;
}

pub(crate) fn walk_linear_term<V, P>(visitor: &mut V, term: &BlockPyTerm<P::Expr>)
where
    V: BlockPyLinearModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: MapExpr<P::Expr>,
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

pub(crate) fn walk_linear_expr<V, P>(visitor: &mut V, expr: &P::Expr)
where
    V: BlockPyLinearModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: MapExpr<P::Expr>,
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
        py_expr!("None")
    }

    fn is_implicit_none_expr(expr: &Self) -> bool {
        matches!(expr, Expr::NoneLiteral(_))
    }
}

impl ImplicitNoneExpr for CoreBlockPyExprWithAwaitAndYield {
    fn implicit_none_expr() -> Self {
        core_runtime_name_expr_with_meta("NONE", Default::default(), Default::default())
    }

    fn is_implicit_none_expr(expr: &Self) -> bool {
        matches!(
            expr,
            CoreBlockPyExprWithAwaitAndYield::Load(op)
                if op.name.is_runtime_symbol("NONE")
        )
    }
}

impl ImplicitNoneExpr for CoreBlockPyExprWithYield {
    fn implicit_none_expr() -> Self {
        core_runtime_name_expr_with_meta("NONE", Default::default(), Default::default())
    }

    fn is_implicit_none_expr(expr: &Self) -> bool {
        matches!(
            expr,
            CoreBlockPyExprWithYield::Load(op) if op.name.is_runtime_symbol("NONE")
        )
    }
}

impl ImplicitNoneExpr for CoreBlockPyExpr {
    fn implicit_none_expr() -> Self {
        core_runtime_name_expr_with_meta("NONE", Default::default(), Default::default())
    }

    fn is_implicit_none_expr(expr: &Self) -> bool {
        matches!(
            expr,
            CoreBlockPyExpr::Load(op) if op.name.is_runtime_symbol("NONE")
        )
    }
}

impl ImplicitNoneExpr for LocatedCoreBlockPyExpr {
    fn implicit_none_expr() -> Self {
        Load::new(LocatedName {
            id: "NONE".into(),
            location: NameLocation::RuntimeName,
        })
        .with_meta(Meta::synthetic())
        .into()
    }

    fn is_implicit_none_expr(expr: &Self) -> bool {
        matches!(
            expr,
            CoreBlockPyExpr::Load(op) if op.name.is_runtime_symbol("NONE")
        )
    }
}

impl ImplicitNoneExpr for CodegenBlockPyExpr {
    fn implicit_none_expr() -> Self {
        Load::new(LocatedName {
            id: "NONE".into(),
            location: NameLocation::RuntimeName,
        })
        .with_meta(Meta::synthetic())
        .into()
    }

    fn is_implicit_none_expr(expr: &Self) -> bool {
        matches!(
            expr,
            CodegenBlockPyExpr::Load(op) if op.name.is_runtime_symbol("NONE")
        )
    }
}

impl<E: ImplicitNoneExpr> BlockPyFallthroughTerm<BlockPyLabel> for BlockPyTerm<E> {
    fn implicit_function_return() -> Self {
        Self::Return(E::implicit_none_expr())
    }
}

#[cfg(test)]
mod test;
