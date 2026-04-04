pub use self::meta::{HasMeta, Meta, WithMeta};
use self::operation_macro::define_operation;
pub use self::param_specs::{Param, ParamDefaultSource, ParamKind, ParamSpec};
pub(crate) use self::scope::{
    build_storage_layout_from_capture_names, compute_make_function_capture_bindings_from_scope,
    compute_storage_layout_from_scope, derive_effective_binding_for_name, ScopeExprNode,
};
pub use self::scope::{
    BindingKind, BindingPurpose, BindingTarget, CallableScopeInfo, CallableScopeKind,
    CellBindingKind, CellCaptureBinding, ClassBodyFallback, ClosureInit, ClosureSlot,
    EffectiveBinding, StorageLayout,
};
use crate::py_expr;
pub use operation::{
    Await, BinOp, BinOpKind, Call, CellRef, CellRefForName, Del, DelItem, GetAttr, GetItem, Load,
    MakeCell, MakeFunction, SetAttr, SetItem, Store, UnaryOp, UnaryOpKind, Yield, YieldFrom,
};
pub use ruff_python_ast::Expr;
use ruff_python_ast::{self as ast};
use soac_macros::{enum_broadcast, match_default, DelegateMatchDefault};
use std::fmt;

pub(crate) mod cfg;
mod convert;
pub(crate) mod dataflow;
mod meta;
mod name_gen;
pub mod operation;
mod operation_macro;
pub(crate) mod param_specs;
pub mod pretty;
pub(crate) mod scope;
pub(crate) mod validate;
pub(crate) use convert::{map_fn, map_module, map_term, try_map_fn, try_map_term};
pub use name_gen::{BlockLabel, FunctionId, FunctionNameGen, ModuleNameGen};
pub(crate) use validate::validate_module;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CounterId(pub usize);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CounterPoint {
    BlockEntry {
        function_id: FunctionId,
        block_label: BlockLabel,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CounterDef {
    pub id: CounterId,
    pub point: CounterPoint,
}
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

pub trait BlockPyNameLike: Clone + fmt::Debug {
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

pub trait MapExpr<In: Instr, Out: Instr> {
    fn map_expr(&mut self, expr: In) -> Out;
    fn map_name(&mut self, name: In::Name) -> Out::Name;
}

pub trait TryMapExpr<In: Instr, Out: Instr, Error> {
    fn try_map_expr(&mut self, expr: In) -> Result<Out, Error>;
    fn try_map_name(&mut self, name: In::Name) -> Result<Out::Name, Error>;
}

pub trait Walkable<E>: Clone + fmt::Debug + Sized {
    fn map_walk(self, f: &mut impl FnMut(E) -> E) -> Self;
    fn walk_mut(&mut self, f: &mut impl FnMut(&mut E));
    fn walk(&self, f: &mut impl FnMut(&E));

    fn walk_try_map<Error>(self, f: &mut impl FnMut(E) -> Result<E, Error>) -> Result<Self, Error>
    where
        E: Clone,
    {
        let mut first_error = None;
        let walked = self.map_walk(&mut |child| {
            if first_error.is_some() {
                return child;
            }

            let original = child.clone();
            match f(child) {
                Ok(mapped) => mapped,
                Err(error) => {
                    first_error = Some(error);
                    original
                }
            }
        });
        match first_error {
            Some(error) => Err(error),
            None => Ok(walked),
        }
    }
}

pub trait Instr: Walkable<Self> + Clone + fmt::Debug + Sized {
    type Name: BlockPyNameLike;
}

pub trait InstrExprNode<I>: Walkable<I> + Sized
where
    I: Instr,
{
    type Mapped<T: Instr>;

    fn map_typed_children<T, M>(self, map: &mut M) -> Self::Mapped<T>
    where
        T: Instr,
        M: MapExpr<I, T>;
    fn try_map_typed_children<T, Error, M>(self, map: &mut M) -> Result<Self::Mapped<T>, Error>
    where
        T: Instr,
        M: TryMapExpr<I, T, Error>;
}

impl BlockPyNameLike for ast::ExprName {
    fn id_str(&self) -> &str {
        self.id.as_str()
    }
}

impl Walkable<Expr> for Expr {
    fn map_walk(self, f: &mut impl FnMut(Self) -> Expr) -> Expr {
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

    fn walk_mut(&mut self, f: &mut impl FnMut(&mut Expr)) {
        struct DirectChildTransformer<'a, F>(&'a mut F);

        impl<F> crate::transformer::Transformer for DirectChildTransformer<'_, F>
        where
            F: FnMut(&mut Expr),
        {
            fn visit_expr(&mut self, expr: &mut Expr) {
                (self.0)(expr);
            }
        }

        let mut transformer = DirectChildTransformer(f);
        crate::transformer::walk_expr(&mut transformer, self);
    }

    fn walk(&self, f: &mut impl FnMut(&Expr)) {
        let mut cloned = self.clone();
        cloned.walk_mut(&mut |expr| f(expr));
    }
}

impl Instr for Expr {
    type Name = ast::ExprName;
}

#[derive(Clone)]
pub enum UnresolvedName {
    SourceName(ast::name::Name),
    RuntimeName(ast::name::Name),
}

impl fmt::Debug for UnresolvedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.pretty_id())
    }
}

impl BlockPyNameLike for UnresolvedName {
    fn id_str(&self) -> &str {
        match self {
            Self::SourceName(name) | Self::RuntimeName(name) => name.as_str(),
        }
    }

    fn is_runtime_name(&self) -> bool {
        matches!(self, Self::RuntimeName(_))
    }
}

impl From<ast::ExprName> for UnresolvedName {
    fn from(value: ast::ExprName) -> Self {
        Self::SourceName(value.id)
    }
}

impl From<ast::name::Name> for UnresolvedName {
    fn from(value: ast::name::Name) -> Self {
        Self::SourceName(value)
    }
}

impl UnresolvedName {
    pub fn name(self) -> ast::name::Name {
        match self {
            Self::SourceName(name) | Self::RuntimeName(name) => name,
        }
    }
}

impl Walkable<RuffExpr> for RuffExpr {
    fn map_walk(self, f: &mut impl FnMut(Self) -> RuffExpr) -> RuffExpr {
        RuffExpr(self.0.map_walk(&mut |expr| f(RuffExpr(expr)).0))
    }

    fn walk_mut(&mut self, f: &mut impl FnMut(&mut RuffExpr)) {
        self.0.walk_mut(&mut |expr| {
            let mut wrapped = RuffExpr(expr.clone());
            f(&mut wrapped);
            *expr = wrapped.0;
        });
    }

    fn walk(&self, f: &mut impl FnMut(&RuffExpr)) {
        self.0.walk(&mut |expr| {
            let wrapped = RuffExpr(expr.clone());
            f(&wrapped);
        });
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct LocatedName {
    pub id: ruff_python_ast::name::Name,
    pub location: NameLocation,
}

impl fmt::Debug for LocatedName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.pretty_id())
    }
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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FunctionKind {
    Function,
    Coroutine,
    Generator,
    AsyncGenerator,
}

#[derive(Debug, Clone)]
pub struct Block<S, T: Instr = S> {
    pub label: BlockLabel,
    pub body: Vec<S>,
    pub term: BlockTerm<T>,
    pub params: Vec<BlockParam>,
    pub exc_edge: Option<BlockEdge>,
}

impl<S, T: Instr> Block<S, T> {
    pub fn label_str(&self) -> String {
        self.label.to_string()
    }
}

impl<S: NormalizedInstr, T: Instr> Block<S, T> {
    pub fn new(
        label: BlockLabel,
        body: Vec<S>,
        term: BlockTerm<T>,
        params: Vec<BlockParam>,
        exc_edge: Option<BlockEdge>,
    ) -> Self {
        let block = Self {
            label,
            body,
            term,
            params,
            exc_edge,
        };
        assert_blockpy_block_normalized(&block);
        block
    }
}

impl<S: NormalizedInstr, T: Instr> Block<S, T>
where
    BlockTerm<T>: BlockPyFallthroughTerm<BlockLabel>,
{
    pub fn from_builder(
        label: BlockLabel,
        builder: BlockBuilder<S, BlockTerm<T>>,
        params: Vec<BlockParam>,
        exc_edge: Option<BlockEdge>,
        fallthrough_target: Option<BlockLabel>,
    ) -> Self {
        Self::new(
            label,
            builder.body,
            builder.term.unwrap_or_else(|| match fallthrough_target {
                Some(target) => BlockTerm::<T>::jump_term(target),
                None => BlockTerm::<T>::implicit_function_return(),
            }),
            params,
            exc_edge,
        )
    }
}

impl<S, T: Instr> Block<S, T> {
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
    pub module_name_gen: ModuleNameGen,
    pub callable_defs: Vec<BlockPyFunction<P, S>>,
    pub module_constants: Vec<CoreBlockPyExpr<LocatedName>>,
    pub counter_defs: Vec<CounterDef>,
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
        debug_assert!(
            self.counter_defs.is_empty(),
            "map_callable_defs does not preserve counter defs"
        );
        BlockPyModule {
            module_name_gen: self.module_name_gen,
            callable_defs: self.callable_defs.into_iter().map(&mut f).collect(),
            module_constants: Vec::new(),
            counter_defs: Vec::new(),
        }
    }
}

#[derive(Clone, derive_more::From, DelegateMatchDefault)]
#[enum_broadcast(HasMeta, WithMeta, Walkable, Debug)]
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
    Await(Await<Self>),
    Yield(Yield<Self>),
    YieldFrom(YieldFrom<Self>),
}

#[derive(Clone, derive_more::From, DelegateMatchDefault)]
#[enum_broadcast(HasMeta, WithMeta, Walkable, Debug)]
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
    Yield(Yield<Self>),
    YieldFrom(YieldFrom<Self>),
}

#[derive(Clone, derive_more::From)]
#[enum_broadcast(HasMeta, WithMeta, Walkable, Debug)]
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

#[derive(Clone, derive_more::From)]
#[enum_broadcast(HasMeta, WithMeta, Walkable, Debug)]
pub enum CodegenBlockPyExpr {
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
    IncrementCounter(IncrementCounter),
    CellRef(CellRef),
    MakeFunction(MakeFunction<Self>),
}

define_operation! {
    pub struct IncrementCounter {
        counter_id: CounterId,
    }
}

#[derive(Clone, derive_more::From)]
pub enum BlockPyLiteral {
    StringLiteral(CoreStringLiteral),
    BytesLiteral(CoreBytesLiteral),
    NumberLiteral(CoreNumberLiteral),
}

impl fmt::Debug for BlockPyLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StringLiteral(value) => value.fmt(f),
            Self::BytesLiteral(value) => value.fmt(f),
            Self::NumberLiteral(value) => value.fmt(f),
        }
    }
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

pub(crate) fn literal_value(literal: impl Into<BlockPyLiteral>, meta: Meta) -> LiteralValue {
    LiteralValue::new(literal.into()).with_meta(meta)
}

pub(crate) fn literal_expr<E>(literal: impl Into<BlockPyLiteral>, meta: Meta) -> E
where
    E: From<LiteralValue>,
{
    E::from(literal_value(literal, meta))
}

#[derive(Clone)]
pub struct CoreStringLiteral {
    pub value: String,
}

impl fmt::Debug for CoreStringLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.value)
    }
}

#[derive(Clone)]
pub struct CoreBytesLiteral {
    pub value: Vec<u8>,
}

impl fmt::Debug for CoreBytesLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.value)
    }
}

#[derive(Clone)]
pub struct CoreNumberLiteral {
    pub value: CoreNumberLiteralValue,
}

impl fmt::Debug for CoreNumberLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt(f)
    }
}

#[derive(Clone)]
pub enum CoreNumberLiteralValue {
    Int(ast::Int),
    Float(f64),
}

impl fmt::Debug for CoreNumberLiteralValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(value) => write!(f, "{value}"),
            Self::Float(value) => write!(f, "{value:?}"),
        }
    }
}

impl Instr for CoreBlockPyExprWithAwaitAndYield {
    type Name = UnresolvedName;
}

impl Instr for CoreBlockPyExprWithYield {
    type Name = UnresolvedName;
}

impl<N: BlockPyNameLike> Instr for CoreBlockPyExpr<N> {
    type Name = N;
}

impl Instr for CodegenBlockPyExpr {
    type Name = LocatedName;
}

pub(crate) fn try_lower_core_expr_without_yield_with_mapper<M>(
    expr: CoreBlockPyExprWithYield,
    map: &mut M,
) -> Result<CoreBlockPyExpr, CoreBlockPyExprWithYield>
where
    M: TryMapExpr<CoreBlockPyExprWithYield, CoreBlockPyExpr, CoreBlockPyExprWithYield>,
{
    match_default!(expr: crate::block_py::CoreBlockPyExprWithYield {
        CoreBlockPyExprWithYield::Yield(node) => Err(node.into()),
        CoreBlockPyExprWithYield::YieldFrom(node) => Err(node.into()),
        rest => Ok(rest.try_map_typed_children(map)?.into()),
    })
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
    let func = Load::new(ast::name::Name::new(func_name))
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
    Load::new(runtime_symbol(name))
        .with_meta(Meta::new(node_index, range))
        .into()
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

pub(crate) fn runtime_symbol(name: &str) -> UnresolvedName {
    UnresolvedName::RuntimeName(name.into())
}

#[derive(Debug, Clone)]
pub enum CoreBlockPyCallArg<E> {
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
pub enum CoreBlockPyKeywordArg<E> {
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
    pub kind: FunctionKind,
    pub params: ParamSpec,
    pub blocks: Vec<Block<S, P::Expr>>,
    pub doc: Option<String>,
    pub storage_layout: Option<StorageLayout>,
    pub scope: CallableScopeInfo,
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
            scope: self.scope.clone(),
        }
    }
}

impl<P: BlockPyPass, S> BlockPyFunction<P, S> {
    pub fn lowered_kind(&self) -> &FunctionKind {
        &self.kind
    }

    pub fn storage_layout(&self) -> &Option<StorageLayout> {
        &self.storage_layout
    }

    pub fn entry_block(&self) -> &Block<S, P::Expr> {
        self.blocks
            .first()
            .expect("BlockPyFunction should have at least one block")
    }

    pub fn map_blocks<Q: BlockPyPass, T>(
        self,
        mut f: impl FnMut(Block<S, P::Expr>) -> Block<T, Q::Expr>,
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
            scope: self.scope,
        }
    }
}

pub trait NormalizedInstr {
    fn assert_blockpy_normalized(&self);
}

pub trait BlockPyPass: Clone + fmt::Debug {
    type Expr: Instr;
}

pub type InstrName<I> = <I as Instr>::Name;
pub type ResolvedStorageBlock = Block<LocatedCoreBlockPyExpr>;
pub type CodegenBlock = Block<CodegenBlockPyExpr>;
pub type CodegenBlockPyFunction = BlockPyFunction<crate::passes::CodegenBlockPyPass>;
pub type CodegenBlockPyModule = BlockPyModule<crate::passes::CodegenBlockPyPass>;

pub(crate) type BlockPyBlock<I> = Block<StructuredInstr<I>, I>;

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
    E: Instr,
    F: FnMut(&E) -> bool,
{
    fn expr_any_impl<E, F>(expr: &E, predicate: &mut F) -> bool
    where
        E: Instr,
        F: FnMut(&E) -> bool,
    {
        if predicate(expr) {
            return true;
        }

        let mut found = false;
        expr.walk(&mut |child| {
            if !found && expr_any_impl(child, predicate) {
                found = true;
            }
        });
        found
    }

    expr_any_impl(expr, &mut predicate)
}

pub fn assert_blockpy_block_normalized<S: NormalizedInstr, T: Instr>(block: &Block<S, T>) {
    for stmt in &block.body {
        stmt.assert_blockpy_normalized();
    }
}

#[derive(Debug, Clone)]
pub struct BlockBuilder<S, T> {
    pub body: Vec<S>,
    pub term: Option<T>,
}

pub(crate) type BlockPyStmtBuilder<I> = BlockBuilder<StructuredInstr<I>, BlockTerm<I>>;

impl<S: NormalizedInstr, T> BlockBuilder<S, T> {
    pub fn new() -> Self {
        Self {
            body: Vec::new(),
            term: None,
        }
    }

    pub fn assert_normalized(&self) {
        for stmt in &self.body {
            stmt.assert_blockpy_normalized();
        }
    }

    pub fn from_stmts(stmts: Vec<S>) -> Self {
        Self::with_term(stmts, None)
    }

    pub fn with_term(body: Vec<S>, term: impl Into<Option<T>>) -> Self {
        let builder = BlockBuilder {
            body,
            term: term.into(),
        };
        builder.assert_normalized();
        builder
    }

    pub fn push_stmt(&mut self, stmt: S) {
        assert!(
            self.term.is_none(),
            "cannot append structured BlockPy stmt after block-builder terminator"
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
            "cannot replace existing block-builder terminator"
        );
        self.term = Some(term);
    }

    pub fn finish(self) -> Self {
        self.assert_normalized();
        self
    }
}

impl<S: NormalizedInstr, T: BlockPyJumpTerm<BlockLabel>> BlockBuilder<S, T> {
    pub fn jump(target: BlockLabel) -> Self {
        Self::with_term(Vec::new(), Some(T::jump_term(target)))
    }
}

#[derive(Debug, Clone)]
pub(crate) enum StructuredInstr<I: Instr> {
    Expr(I),
    If(StructuredIf<I>),
}

impl<I: Instr> From<I> for StructuredInstr<I> {
    fn from(value: I) -> Self {
        Self::Expr(value)
    }
}

impl<I: Instr> StructuredInstr<I> {
    pub fn assert_normalized(&self) {
        if let Self::If(if_stmt) = self {
            if_stmt.body.assert_normalized();
            if_stmt.orelse.assert_normalized();
        }
    }
}

impl<I: Instr> NormalizedInstr for StructuredInstr<I> {
    fn assert_blockpy_normalized(&self) {
        self.assert_normalized();
    }
}

impl<I> NormalizedInstr for I
where
    I: Instr,
{
    fn assert_blockpy_normalized(&self) {}
}

pub(crate) fn convert_structured_instr_expr<IIn, IOut>(
    value: StructuredInstr<IIn>,
) -> StructuredInstr<IOut>
where
    IIn: Instr,
    IOut: Instr + From<IIn>,
{
    match value {
        StructuredInstr::Expr(expr) => StructuredInstr::Expr(expr.into()),
        StructuredInstr::If(if_stmt) => StructuredInstr::If(StructuredIf {
            test: if_stmt.test.into(),
            body: convert_block_builder_expr(if_stmt.body),
            orelse: convert_block_builder_expr(if_stmt.orelse),
        }),
    }
}

#[derive(Debug, Clone)]
pub enum BlockTerm<I: Instr> {
    Jump(BlockEdge),
    IfTerm(TermIf<I>),
    BranchTable(TermBranchTable<I>),
    Raise(TermRaise<I>),
    Return(I),
}

#[derive(Debug, Clone)]
pub(crate) struct StructuredIf<I: Instr> {
    pub test: I,
    pub body: BlockBuilder<StructuredInstr<I>, BlockTerm<I>>,
    pub orelse: BlockBuilder<StructuredInstr<I>, BlockTerm<I>>,
}

#[derive(Debug, Clone)]
pub struct TermIf<I: Instr> {
    pub test: I,
    pub then_label: BlockLabel,
    pub else_label: BlockLabel,
}

#[derive(Debug, Clone)]
pub struct TermBranchTable<I: Instr> {
    pub index: I,
    pub targets: Vec<BlockLabel>,
    pub default_label: BlockLabel,
}

#[derive(Debug, Clone)]
pub struct TermRaise<I: Instr> {
    pub exc: Option<I>,
}

pub(crate) fn convert_block_builder_expr<IIn, IOut>(
    value: BlockBuilder<StructuredInstr<IIn>, BlockTerm<IIn>>,
) -> BlockBuilder<StructuredInstr<IOut>, BlockTerm<IOut>>
where
    IIn: Instr,
    IOut: Instr + From<IIn>,
{
    BlockBuilder {
        body: value
            .body
            .into_iter()
            .map(convert_structured_instr_expr)
            .collect(),
        term: value.term.map(convert_blockpy_term_expr),
    }
}

pub fn convert_blockpy_term_expr<IIn, IOut>(value: BlockTerm<IIn>) -> BlockTerm<IOut>
where
    IIn: Instr,
    IOut: Instr + From<IIn>,
{
    match value {
        BlockTerm::Jump(edge) => BlockTerm::Jump(edge),
        BlockTerm::IfTerm(if_term) => BlockTerm::IfTerm(TermIf {
            test: if_term.test.into(),
            then_label: if_term.then_label,
            else_label: if_term.else_label,
        }),
        BlockTerm::BranchTable(branch) => BlockTerm::BranchTable(TermBranchTable {
            index: branch.index.into(),
            targets: branch.targets,
            default_label: branch.default_label,
        }),
        BlockTerm::Raise(raise_stmt) => BlockTerm::Raise(TermRaise {
            exc: raise_stmt.exc.map(Into::into),
        }),
        BlockTerm::Return(value) => BlockTerm::Return(value.into()),
    }
}

#[derive(Debug, Clone)]
pub struct BlockEdge {
    pub target: BlockLabel,
    pub args: Vec<BlockArg>,
}

impl BlockEdge {
    pub fn new(target: BlockLabel) -> Self {
        Self {
            target,
            args: Vec::new(),
        }
    }

    pub fn with_args(target: BlockLabel, args: Vec<BlockArg>) -> Self {
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

enum TermChildRef<'a, E> {
    Expr(&'a E),
    Label(&'a BlockLabel),
}

fn walk_term_children<I: Instr>(
    term: &BlockTerm<I>,
    visit_child: &mut impl FnMut(TermChildRef<'_, I>),
) {
    match term {
        BlockTerm::Jump(edge) => {
            visit_child(TermChildRef::Label(&edge.target));
        }
        BlockTerm::IfTerm(if_term) => {
            visit_child(TermChildRef::Expr(&if_term.test));
            visit_child(TermChildRef::Label(&if_term.then_label));
            visit_child(TermChildRef::Label(&if_term.else_label));
        }
        BlockTerm::BranchTable(branch) => {
            visit_child(TermChildRef::Expr(&branch.index));
            for target in &branch.targets {
                visit_child(TermChildRef::Label(target));
            }
            visit_child(TermChildRef::Label(&branch.default_label));
        }
        BlockTerm::Raise(raise_stmt) => {
            if let Some(exc) = &raise_stmt.exc {
                visit_child(TermChildRef::Expr(exc));
            }
        }
        BlockTerm::Return(value) => visit_child(TermChildRef::Expr(value)),
    }
}

fn walk_expr_children<E>(expr: &E, visit_expr: &mut impl FnMut(&E))
where
    E: Walkable<E>,
{
    expr.walk(visit_expr);
}

pub(crate) trait BlockPyLinearModuleVisitor<P>
where
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    fn visit_module(&mut self, module: &BlockPyModule<P>) {
        walk_linear_module(self, module);
    }

    fn visit_fn(&mut self, func: &BlockPyFunction<P>) {
        walk_linear_fn(self, func);
    }

    fn visit_block(&mut self, block: &Block<P::Expr, P::Expr>) {
        walk_linear_block(self, block);
    }

    fn visit_stmt(&mut self, stmt: &P::Expr) {
        walk_linear_stmt(self, stmt);
    }

    fn visit_term(&mut self, term: &BlockTerm<P::Expr>) {
        walk_linear_term(self, term);
    }

    fn visit_label(&mut self, label: &BlockLabel) {
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
    P::Expr: Walkable<P::Expr>,
{
    for function in &module.callable_defs {
        visitor.visit_fn(function);
    }
}

pub(crate) fn walk_linear_fn<V, P>(visitor: &mut V, func: &BlockPyFunction<P>)
where
    V: BlockPyLinearModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    for block in &func.blocks {
        visitor.visit_block(block);
    }
}

pub(crate) fn walk_linear_block<V, P>(visitor: &mut V, block: &Block<P::Expr, P::Expr>)
where
    V: BlockPyLinearModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
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
    P::Expr: Walkable<P::Expr>,
{
    visitor.visit_expr(stmt);
}

pub(crate) fn walk_linear_label<V, P>(visitor: &mut V, label: &BlockLabel)
where
    V: BlockPyLinearModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    let _ = visitor;
    let _ = label;
}

pub(crate) fn walk_linear_term<V, P>(visitor: &mut V, term: &BlockTerm<P::Expr>)
where
    V: BlockPyLinearModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    walk_term_children(term, &mut |child| match child {
        TermChildRef::Expr(expr) => visitor.visit_expr(expr),
        TermChildRef::Label(label) => visitor.visit_label(label),
    });
}

pub(crate) fn walk_linear_expr<V, P>(visitor: &mut V, expr: &P::Expr)
where
    V: BlockPyLinearModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    walk_expr_children(expr, &mut |child| visitor.visit_expr(child));
}

#[cfg(test)]
pub(crate) trait BlockPyModuleVisitor<P>
where
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    fn visit_module(&mut self, module: &BlockPyModule<P, StructuredInstr<P::Expr>>) {
        walk_module(self, module);
    }

    fn visit_fn(&mut self, func: &BlockPyFunction<P, StructuredInstr<P::Expr>>) {
        walk_fn(self, func);
    }

    fn visit_block(&mut self, block: &Block<StructuredInstr<P::Expr>, P::Expr>) {
        walk_block(self, block);
    }

    fn visit_fragment(
        &mut self,
        fragment: &BlockBuilder<StructuredInstr<P::Expr>, BlockTerm<P::Expr>>,
    ) {
        walk_fragment(self, fragment);
    }

    fn visit_stmt(&mut self, stmt: &StructuredInstr<P::Expr>) {
        walk_stmt(self, stmt);
    }

    fn visit_term(&mut self, term: &BlockTerm<P::Expr>) {
        walk_term(self, term);
    }

    fn visit_label(&mut self, label: &BlockLabel) {
        walk_label::<Self, P>(self, label);
    }

    fn visit_expr(&mut self, expr: &P::Expr) {
        walk_expr(self, expr);
    }
}

#[cfg(test)]
pub(crate) fn walk_module<V, P>(
    visitor: &mut V,
    module: &BlockPyModule<P, StructuredInstr<P::Expr>>,
) where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    for function in &module.callable_defs {
        visitor.visit_fn(function);
    }
}

#[cfg(test)]
pub(crate) fn walk_fn<V, P>(visitor: &mut V, func: &BlockPyFunction<P, StructuredInstr<P::Expr>>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    for block in &func.blocks {
        visitor.visit_block(block);
    }
}

#[cfg(test)]
pub(crate) fn walk_block<V, P>(visitor: &mut V, block: &Block<StructuredInstr<P::Expr>, P::Expr>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    for stmt in &block.body {
        visitor.visit_stmt(stmt);
    }
    if let Some(exc_edge) = &block.exc_edge {
        visitor.visit_label(&exc_edge.target);
    }
    visitor.visit_term(&block.term);
}

#[cfg(test)]
pub(crate) fn walk_fragment<V, P>(
    visitor: &mut V,
    fragment: &BlockBuilder<StructuredInstr<P::Expr>, BlockTerm<P::Expr>>,
) where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    for stmt in &fragment.body {
        visitor.visit_stmt(stmt);
    }
    if let Some(term) = &fragment.term {
        visitor.visit_term(term);
    }
}

#[cfg(test)]
pub(crate) fn walk_stmt<V, P>(visitor: &mut V, stmt: &StructuredInstr<P::Expr>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    match stmt {
        StructuredInstr::Expr(expr) => visitor.visit_expr(expr),
        StructuredInstr::If(if_stmt) => {
            visitor.visit_expr(&if_stmt.test);
            visitor.visit_fragment(&if_stmt.body);
            visitor.visit_fragment(&if_stmt.orelse);
        }
    }
}

#[cfg(test)]
pub(crate) fn walk_label<V, P>(visitor: &mut V, label: &BlockLabel)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    let _ = visitor;
    let _ = label;
}

#[cfg(test)]
pub(crate) fn walk_term<V, P>(visitor: &mut V, term: &BlockTerm<P::Expr>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    walk_term_children(term, &mut |child| match child {
        TermChildRef::Expr(expr) => visitor.visit_expr(expr),
        TermChildRef::Label(label) => visitor.visit_label(label),
    });
}

#[cfg(test)]
pub(crate) fn walk_expr<V, P>(visitor: &mut V, expr: &P::Expr)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    P::Expr: Walkable<P::Expr>,
{
    walk_expr_children(expr, &mut |child| visitor.visit_expr(child));
}

impl<I: Instr> BlockPyJumpTerm<BlockLabel> for BlockTerm<I> {
    fn jump_term(target: BlockLabel) -> Self {
        Self::Jump(BlockEdge::new(target))
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
        .into()
    }

    fn is_implicit_none_expr(expr: &Self) -> bool {
        matches!(
            expr,
            LocatedCoreBlockPyExpr::Load(op) if op.name.is_runtime_symbol("NONE")
        )
    }
}

impl ImplicitNoneExpr for CodegenBlockPyExpr {
    fn implicit_none_expr() -> Self {
        Load::new(LocatedName {
            id: "NONE".into(),
            location: NameLocation::RuntimeName,
        })
        .into()
    }

    fn is_implicit_none_expr(expr: &Self) -> bool {
        matches!(
            expr,
            CodegenBlockPyExpr::Load(op) if op.name.is_runtime_symbol("NONE")
        )
    }
}

impl<I: Instr + ImplicitNoneExpr> BlockPyFallthroughTerm<BlockLabel> for BlockTerm<I> {
    fn implicit_function_return() -> Self {
        Self::Return(I::implicit_none_expr())
    }
}

#[cfg(test)]
mod test;
