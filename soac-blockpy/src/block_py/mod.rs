pub use self::meta::{HasMeta, Meta, WithMeta};
use self::operation as block_py_operation;
pub use self::param_specs::{Param, ParamDefaultSource, ParamKind, ParamSpec};
pub(crate) use self::semantics::{
    build_storage_layout_from_capture_names, compute_storage_layout_from_semantics,
    derive_effective_binding_for_name, BlockPySemanticExprNode,
};
pub use self::semantics::{
    BindingTarget, BlockPyBindingKind, BlockPyBindingPurpose, BlockPyCallableScopeKind,
    BlockPyCallableSemanticInfo, BlockPyCellBindingKind, BlockPyClassBodyFallback,
    BlockPyEffectiveBinding, ClosureInit, ClosureSlot, StorageLayout,
};
use crate::passes::{CodegenBlockPyPass, ResolvedStorageBlockPyPass, RuffBlockPyPass};
use crate::py_expr;
pub use operation::{
    BinOp, BinOpKind, CellRef, CellRefTarget, DelDeref, DelDerefQuietly, DelItem, DelQuietly,
    GetAttr, GetItem, InplaceBinOp, InplaceBinOpKind, LoadCell, LoadGlobal, MakeCell, MakeFunction,
    MakeString, Operation, OperationDetail, SetAttr, SetItem, StoreCell, StoreGlobal, TernaryOp,
    TernaryOpKind, UnaryOp, UnaryOpKind,
};
pub use ruff_python_ast::Expr;
use ruff_python_ast::{self as ast, ExprName};
use ruff_text_size::TextRange;
use std::fmt;

pub(crate) mod cfg;
mod convert;
pub(crate) mod dataflow;
pub(crate) mod exception;
mod meta;
mod name_gen;
pub mod operation;
pub(crate) mod param_specs;
pub mod pretty;
pub(crate) mod semantics;
pub(crate) mod state;
pub(crate) mod validate;
pub(crate) use convert::BlockPyModuleMap;
#[cfg(test)]
pub(crate) use convert::BlockPyModuleTryMap;
pub(crate) use convert::{
    map_call_args_with, map_keyword_args_with, try_map_call_args_with, try_map_keyword_args_with,
};
pub use name_gen::{FunctionNameGen, ModuleNameGen};
pub(crate) use validate::validate_module;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockPyLabel {
    index: u32,
}

impl BlockPyLabel {
    pub(crate) fn from_u32_index(value: u32) -> Self {
        Self { index: value }
    }

    pub(crate) fn from_index(value: usize) -> Self {
        Self::from_u32_index(u32::try_from(value).expect("block label usize should fit in u32"))
    }
}

#[cfg(test)]
impl From<u32> for BlockPyLabel {
    fn from(value: u32) -> Self {
        Self::from_u32_index(value)
    }
}

#[cfg(test)]
impl From<usize> for BlockPyLabel {
    fn from(value: usize) -> Self {
        Self::from_index(value)
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
    name.starts_with("_dp_") || name.starts_with("__dp_") || name == "runtime"
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
    fn pretty_id(&self) -> String {
        self.id_str().to_string()
    }
    fn range(&self) -> TextRange;
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

pub trait TryMapExpr<T, Error>: Clone + fmt::Debug + Into<Expr> + Sized {
    fn try_map_expr(self, f: &mut impl FnMut(Self) -> Result<T, Error>) -> Result<T, Error>;
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

    pub fn resolved_pretty_id(&self) -> String {
        match self.location {
            NameLocation::Local { slot } => format!("local slot {slot}"),
            NameLocation::Global => self.id.as_str().to_string(),
            NameLocation::OwnedCell { slot } => format!("owned cell slot {slot}"),
            NameLocation::ClosureCell { slot } => format!("closure slot {slot}"),
            NameLocation::CapturedCellSource { slot } => {
                format!("captured cell source slot {slot}")
            }
        }
    }
}

impl BlockPyNameLike for LocatedName {
    fn id_str(&self) -> &str {
        self.id.as_str()
    }

    fn pretty_id(&self) -> String {
        self.resolved_pretty_id()
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
pub enum BlockPyFunctionKind {
    Function,
    Coroutine,
    Generator,
    AsyncGenerator,
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

pub(crate) fn move_entry_block_to_front<S, T>(
    blocks: &mut Vec<CfgBlock<S, T>>,
    entry_label: BlockPyLabel,
) {
    if let Some(entry_index) = blocks.iter().position(|block| block.label == entry_label) {
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
    Op(Box<Operation<Self, ast::ExprName>>),
    Call(CoreBlockPyCall<Self>),
    Await(CoreBlockPyAwait<Self>),
    Yield(CoreBlockPyYield<Self>),
    YieldFrom(CoreBlockPyYieldFrom<Self>),
}

#[derive(Debug, Clone)]
pub enum CoreBlockPyExprWithYield {
    Name(ast::ExprName),
    Literal(CoreBlockPyLiteral),
    Op(Box<Operation<Self, ast::ExprName>>),
    Call(CoreBlockPyCall<Self>),
    Yield(CoreBlockPyYield<Self>),
    YieldFrom(CoreBlockPyYieldFrom<Self>),
}

#[derive(Debug, Clone)]
pub enum CoreBlockPyExpr<N = ast::ExprName> {
    Name(N),
    Literal(CoreBlockPyLiteral),
    Op(Box<Operation<Self, N>>),
    Call(CoreBlockPyCall<Self>),
}

pub type LocatedCoreBlockPyExpr = CoreBlockPyExpr<LocatedName>;

#[derive(Debug, Clone)]
pub enum CodegenBlockPyExpr<N = ast::ExprName> {
    Name(N),
    Literal(CodegenBlockPyLiteral),
    Op(Box<Operation<Self, N>>),
    Call(CoreBlockPyCall<Self>),
}

pub type LocatedCodegenBlockPyExpr = CodegenBlockPyExpr<LocatedName>;

#[derive(Debug, Clone)]
pub enum CoreBlockPyLiteral {
    StringLiteral(CoreStringLiteral),
    BytesLiteral(CoreBytesLiteral),
    NumberLiteral(CoreNumberLiteral),
}

#[derive(Debug, Clone)]
pub enum CodegenBlockPyLiteral {
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

pub(crate) trait CoreCallLikeExpr: Sized {
    type Name: BlockPyNameLike + From<ast::ExprName>;

    fn from_name(name: ast::ExprName) -> Self;

    fn from_call(call: CoreBlockPyCall<Self>) -> Self;

    fn from_operation(operation: block_py_operation::Operation<Self, Self::Name>) -> Self;
}

impl CoreCallLikeExpr for CoreBlockPyExprWithAwaitAndYield {
    type Name = ast::ExprName;

    fn from_name(name: ast::ExprName) -> Self {
        Self::Name(name)
    }

    fn from_call(call: CoreBlockPyCall<Self>) -> Self {
        Self::Call(call)
    }

    fn from_operation(
        operation: block_py_operation::Operation<
            Self,
            <CoreBlockPyExprWithAwaitAndYield as CoreCallLikeExpr>::Name,
        >,
    ) -> Self {
        Self::Op(Box::new(operation))
    }
}

impl MapExpr<CoreBlockPyExprWithAwaitAndYield> for CoreBlockPyExprWithAwaitAndYield {
    fn map_expr(
        self,
        f: &mut impl FnMut(Self) -> CoreBlockPyExprWithAwaitAndYield,
    ) -> CoreBlockPyExprWithAwaitAndYield {
        match self {
            Self::Name(_) | Self::Literal(_) => self,
            Self::Op(operation) => Self::Op(Box::new(operation.map_expr(&mut *f))),
            Self::Call(call) => Self::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(f(*call.func)),
                args: map_call_args_with(call.args, &mut *f),
                keywords: map_keyword_args_with(call.keywords, &mut *f),
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

impl MapExpr<CoreBlockPyExprWithYield> for CoreBlockPyExprWithAwaitAndYield {
    fn map_expr(
        self,
        f: &mut impl FnMut(Self) -> CoreBlockPyExprWithYield,
    ) -> CoreBlockPyExprWithYield {
        match self {
            Self::Name(name) => CoreBlockPyExprWithYield::Name(name),
            Self::Literal(literal) => CoreBlockPyExprWithYield::Literal(literal),
            Self::Op(operation) => {
                CoreBlockPyExprWithYield::Op(Box::new(operation.map_expr(&mut *f)))
            }
            Self::Call(call) => CoreBlockPyExprWithYield::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(f(*call.func)),
                args: map_call_args_with(call.args, &mut *f),
                keywords: map_keyword_args_with(call.keywords, &mut *f),
            }),
            Self::Await(await_expr) => CoreBlockPyExprWithYield::YieldFrom(CoreBlockPyYieldFrom {
                node_index: await_expr.node_index.clone(),
                range: await_expr.range,
                value: Box::new(core_positional_call_expr_with_meta(
                    "__dp_await_iter",
                    await_expr.node_index,
                    await_expr.range,
                    vec![f(*await_expr.value)],
                )),
            }),
            Self::Yield(yield_expr) => CoreBlockPyExprWithYield::Yield(CoreBlockPyYield {
                node_index: yield_expr.node_index,
                range: yield_expr.range,
                value: yield_expr.value.map(|value| Box::new(f(*value))),
            }),
            Self::YieldFrom(yield_from_expr) => {
                CoreBlockPyExprWithYield::YieldFrom(CoreBlockPyYieldFrom {
                    node_index: yield_from_expr.node_index,
                    range: yield_from_expr.range,
                    value: Box::new(f(*yield_from_expr.value)),
                })
            }
        }
    }
}

impl TryMapExpr<CoreBlockPyExprWithYield, CoreBlockPyExprWithAwaitAndYield>
    for CoreBlockPyExprWithAwaitAndYield
{
    fn try_map_expr(
        self,
        f: &mut impl FnMut(Self) -> Result<CoreBlockPyExprWithYield, CoreBlockPyExprWithAwaitAndYield>,
    ) -> Result<CoreBlockPyExprWithYield, CoreBlockPyExprWithAwaitAndYield> {
        match self {
            Self::Name(name) => Ok(CoreBlockPyExprWithYield::Name(name)),
            Self::Literal(literal) => Ok(CoreBlockPyExprWithYield::Literal(literal)),
            Self::Op(operation) => Ok(CoreBlockPyExprWithYield::Op(Box::new(
                operation.try_map_expr(&mut *f)?,
            ))),
            Self::Call(call) => Ok(CoreBlockPyExprWithYield::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(f(*call.func)?),
                args: try_map_call_args_with(call.args, &mut *f)?,
                keywords: try_map_keyword_args_with(call.keywords, &mut *f)?,
            })),
            Self::Await(_) => Err(self),
            Self::Yield(yield_expr) => Ok(CoreBlockPyExprWithYield::Yield(CoreBlockPyYield {
                node_index: yield_expr.node_index,
                range: yield_expr.range,
                value: yield_expr
                    .value
                    .map(|value| f(*value).map(Box::new))
                    .transpose()?,
            })),
            Self::YieldFrom(yield_from_expr) => {
                Ok(CoreBlockPyExprWithYield::YieldFrom(CoreBlockPyYieldFrom {
                    node_index: yield_from_expr.node_index,
                    range: yield_from_expr.range,
                    value: Box::new(f(*yield_from_expr.value)?),
                }))
            }
        }
    }
}

impl CoreCallLikeExpr for CoreBlockPyExprWithYield {
    type Name = ast::ExprName;

    fn from_name(name: ast::ExprName) -> Self {
        Self::Name(name)
    }

    fn from_call(call: CoreBlockPyCall<Self>) -> Self {
        Self::Call(call)
    }

    fn from_operation(
        operation: block_py_operation::Operation<
            Self,
            <CoreBlockPyExprWithYield as CoreCallLikeExpr>::Name,
        >,
    ) -> Self {
        Self::Op(Box::new(operation))
    }
}

impl MapExpr<CoreBlockPyExprWithYield> for CoreBlockPyExprWithYield {
    fn map_expr(
        self,
        f: &mut impl FnMut(Self) -> CoreBlockPyExprWithYield,
    ) -> CoreBlockPyExprWithYield {
        match self {
            Self::Name(_) | Self::Literal(_) => self,
            Self::Op(operation) => Self::Op(Box::new(operation.map_expr(&mut *f))),
            Self::Call(call) => Self::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(f(*call.func)),
                args: map_call_args_with(call.args, &mut *f),
                keywords: map_keyword_args_with(call.keywords, &mut *f),
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

impl TryMapExpr<CoreBlockPyExpr, CoreBlockPyExprWithYield> for CoreBlockPyExprWithYield {
    fn try_map_expr(
        self,
        f: &mut impl FnMut(Self) -> Result<CoreBlockPyExpr, CoreBlockPyExprWithYield>,
    ) -> Result<CoreBlockPyExpr, CoreBlockPyExprWithYield> {
        match self {
            Self::Name(name) => Ok(CoreBlockPyExpr::Name(name.into())),
            Self::Literal(literal) => Ok(CoreBlockPyExpr::Literal(literal)),
            Self::Op(operation) => Ok(CoreBlockPyExpr::Op(Box::new(
                operation.try_map_expr(&mut *f)?,
            ))),
            Self::Call(call) => Ok(CoreBlockPyExpr::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(f(*call.func)?),
                args: try_map_call_args_with(call.args, &mut *f)?,
                keywords: try_map_keyword_args_with(call.keywords, &mut *f)?,
            })),
            Self::Yield(_) | Self::YieldFrom(_) => Err(self),
        }
    }
}

impl<N: BlockPyNameLike> CoreCallLikeExpr for CoreBlockPyExpr<N> {
    type Name = N;

    fn from_name(name: ast::ExprName) -> Self {
        Self::Name(name.into())
    }

    fn from_call(call: CoreBlockPyCall<Self>) -> Self {
        Self::Call(call)
    }

    fn from_operation(
        operation: block_py_operation::Operation<
            Self,
            <CoreBlockPyExpr<N> as CoreCallLikeExpr>::Name,
        >,
    ) -> Self {
        Self::Op(Box::new(operation))
    }
}

impl<NIn, NOut> MapExpr<CoreBlockPyExpr<NOut>> for CoreBlockPyExpr<NIn>
where
    NIn: BlockPyNameLike,
    NOut: BlockPyNameLike + From<NIn>,
{
    fn map_expr(self, f: &mut impl FnMut(Self) -> CoreBlockPyExpr<NOut>) -> CoreBlockPyExpr<NOut> {
        match self {
            Self::Name(name) => CoreBlockPyExpr::Name(NOut::from(name)),
            Self::Literal(literal) => CoreBlockPyExpr::Literal(literal),
            Self::Op(operation) => CoreBlockPyExpr::Op(Box::new(
                operation.map_expr_and_name(&mut *f, &mut NOut::from),
            )),
            Self::Call(call) => CoreBlockPyExpr::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(f(*call.func)),
                args: map_call_args_with(call.args, &mut *f),
                keywords: map_keyword_args_with(call.keywords, &mut *f),
            }),
        }
    }
}

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
            Self::Name(name) => Ok(CoreBlockPyExpr::Name(NOut::from(name))),
            Self::Literal(literal) => Ok(CoreBlockPyExpr::Literal(literal)),
            Self::Op(operation) => Ok(CoreBlockPyExpr::Op(Box::new(
                operation.try_map_expr_and_name(&mut *f, &mut |name| Ok(NOut::from(name)))?,
            ))),
            Self::Call(call) => Ok(CoreBlockPyExpr::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(f(*call.func)?),
                args: try_map_call_args_with(call.args, &mut *f)?,
                keywords: try_map_keyword_args_with(call.keywords, &mut *f)?,
            })),
        }
    }
}

impl<N: BlockPyNameLike> CoreCallLikeExpr for CodegenBlockPyExpr<N> {
    type Name = N;

    fn from_name(name: ast::ExprName) -> Self {
        Self::Name(name.into())
    }

    fn from_call(call: CoreBlockPyCall<Self>) -> Self {
        Self::Call(call)
    }

    fn from_operation(
        operation: block_py_operation::Operation<
            Self,
            <CodegenBlockPyExpr<N> as CoreCallLikeExpr>::Name,
        >,
    ) -> Self {
        Self::Op(Box::new(operation))
    }
}

impl<NIn, NOut> MapExpr<CodegenBlockPyExpr<NOut>> for CoreBlockPyExpr<NIn>
where
    NIn: BlockPyNameLike,
    NOut: BlockPyNameLike + From<NIn>,
{
    fn map_expr(
        self,
        f: &mut impl FnMut(Self) -> CodegenBlockPyExpr<NOut>,
    ) -> CodegenBlockPyExpr<NOut> {
        match self {
            Self::Name(name) => CodegenBlockPyExpr::Name(NOut::from(name)),
            Self::Literal(CoreBlockPyLiteral::BytesLiteral(literal)) => {
                CodegenBlockPyExpr::Literal(CodegenBlockPyLiteral::BytesLiteral(literal))
            }
            Self::Literal(CoreBlockPyLiteral::NumberLiteral(literal)) => {
                CodegenBlockPyExpr::Literal(CodegenBlockPyLiteral::NumberLiteral(literal))
            }
            Self::Literal(CoreBlockPyLiteral::StringLiteral(_)) => {
                unreachable!("codegen mapping should lower string literals explicitly")
            }
            Self::Op(operation) => CodegenBlockPyExpr::Op(Box::new(
                operation.map_expr_and_name(&mut *f, &mut NOut::from),
            )),
            Self::Call(call) => CodegenBlockPyExpr::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(f(*call.func)),
                args: map_call_args_with(call.args, &mut *f),
                keywords: map_keyword_args_with(call.keywords, &mut *f),
            }),
        }
    }
}

impl<NIn, NOut, Error> TryMapExpr<CodegenBlockPyExpr<NOut>, Error> for CodegenBlockPyExpr<NIn>
where
    NIn: BlockPyNameLike,
    NOut: BlockPyNameLike + From<NIn>,
{
    fn try_map_expr(
        self,
        f: &mut impl FnMut(Self) -> Result<CodegenBlockPyExpr<NOut>, Error>,
    ) -> Result<CodegenBlockPyExpr<NOut>, Error> {
        match self {
            Self::Name(name) => Ok(CodegenBlockPyExpr::Name(NOut::from(name))),
            Self::Literal(literal) => Ok(CodegenBlockPyExpr::Literal(literal)),
            Self::Op(operation) => Ok(CodegenBlockPyExpr::Op(Box::new(
                operation.try_map_expr_and_name(&mut *f, &mut |name| Ok(NOut::from(name)))?,
            ))),
            Self::Call(call) => Ok(CodegenBlockPyExpr::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(f(*call.func)?),
                args: try_map_call_args_with(call.args, &mut *f)?,
                keywords: try_map_keyword_args_with(call.keywords, &mut *f)?,
            })),
        }
    }
}

impl<NIn, NOut> MapExpr<CodegenBlockPyExpr<NOut>> for CodegenBlockPyExpr<NIn>
where
    NIn: BlockPyNameLike,
    NOut: BlockPyNameLike + From<NIn>,
{
    fn map_expr(
        self,
        f: &mut impl FnMut(Self) -> CodegenBlockPyExpr<NOut>,
    ) -> CodegenBlockPyExpr<NOut> {
        match self {
            Self::Name(name) => CodegenBlockPyExpr::Name(NOut::from(name)),
            Self::Literal(literal) => CodegenBlockPyExpr::Literal(literal),
            Self::Op(operation) => CodegenBlockPyExpr::Op(Box::new(
                operation.map_expr_and_name(&mut *f, &mut NOut::from),
            )),
            Self::Call(call) => CodegenBlockPyExpr::Call(CoreBlockPyCall {
                node_index: call.node_index,
                range: call.range,
                func: Box::new(f(*call.func)),
                args: map_call_args_with(call.args, &mut *f),
                keywords: map_keyword_args_with(call.keywords, &mut *f),
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

pub(crate) fn core_operation_expr<E: CoreCallLikeExpr>(
    operation: block_py_operation::Operation<E, E::Name>,
) -> E {
    E::from_operation(operation)
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

#[derive(Debug)]
pub struct BlockPyFunction<P: BlockPyPass> {
    pub function_id: FunctionId,
    pub name_gen: FunctionNameGen,
    pub names: FunctionName,
    pub kind: BlockPyFunctionKind,
    pub params: ParamSpec,
    pub blocks: Vec<CfgBlock<P::Stmt, BlockPyTerm<P::Expr>>>,
    pub doc: Option<String>,
    pub storage_layout: Option<StorageLayout>,
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
            storage_layout: self.storage_layout.clone(),
            semantic: self.semantic.clone(),
        }
    }
}

impl<P: BlockPyPass> BlockPyFunction<P> {
    pub fn lowered_kind(&self) -> &BlockPyFunctionKind {
        &self.kind
    }

    pub fn storage_layout(&self) -> &Option<StorageLayout> {
        &self.storage_layout
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
            storage_layout: self.storage_layout,
            semantic: self.semantic,
        }
    }
}

pub trait BlockPyNormalizedStmt {
    fn assert_blockpy_normalized(&self);
}

pub(crate) trait IntoStructuredBlockPyStmt<E, N>: Clone + fmt::Debug {
    fn into_structured_stmt(self) -> StructuredBlockPyStmt<E, N>;
}

pub trait BlockPyPass: Clone + fmt::Debug {
    type Name: BlockPyNameLike;
    type Expr: BlockPyExprLike;
    type Stmt: BlockPyNormalizedStmt + Clone + fmt::Debug;
}

pub type PassExpr<P> = <P as BlockPyPass>::Expr;
pub type PassName<P> = <P as BlockPyPass>::Name;
pub(crate) type StructuredPassStmt<P> = StructuredBlockPyStmt<PassExpr<P>, PassName<P>>;
pub type PassBlock<P> = CfgBlock<<P as BlockPyPass>::Stmt, BlockPyTerm<PassExpr<P>>>;
pub type ResolvedStorageBlock = PassBlock<ResolvedStorageBlockPyPass>;
pub type CodegenBlock = PassBlock<CodegenBlockPyPass>;

pub type BlockPyCfgBlock<S, T> = CfgBlock<S, T>;
pub(crate) type BlockPyBlock<E = Expr, N = ExprName> =
    BlockPyCfgBlock<StructuredBlockPyStmt<E, N>, BlockPyTerm<E>>;
pub(crate) type BlockPyStructuredIf<E = Expr, N = ExprName> =
    BlockPyIf<E, StructuredBlockPyStmt<E, N>, BlockPyTerm<E>>;

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

pub(crate) type BlockPyStmtFragment<E = Expr, N = ExprName> =
    BlockPyCfgFragment<StructuredBlockPyStmt<E, N>, BlockPyTerm<E>>;

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

pub(crate) type BlockPyStmtFragmentBuilder<E = Expr, N = ExprName> =
    BlockPyCfgFragmentBuilder<StructuredBlockPyStmt<E, N>, BlockPyTerm<E>>;

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

pub(crate) type BlockPyBlockBuilder<E = Expr, N = ExprName> =
    BlockPyCfgBlockBuilder<StructuredBlockPyStmt<E, N>, BlockPyTerm<E>>;

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
pub(crate) enum StructuredBlockPyStmt<E = Expr, N = ExprName> {
    Assign(BlockPyAssign<E, N>),
    Expr(E),
    Delete(BlockPyDelete<N>),
    If(BlockPyStructuredIf<E, N>),
}

impl<E: std::fmt::Debug, N: std::fmt::Debug> StructuredBlockPyStmt<E, N> {
    pub fn assert_normalized(&self) {
        if let Self::If(if_stmt) = self {
            if_stmt.body.assert_normalized();
            if_stmt.orelse.assert_normalized();
        }
    }
}

impl<E: std::fmt::Debug, N: std::fmt::Debug> BlockPyNormalizedStmt for StructuredBlockPyStmt<E, N> {
    fn assert_blockpy_normalized(&self) {
        self.assert_normalized();
    }
}

pub(crate) fn convert_blockpy_stmt_expr<EIn, EOut, N>(
    value: StructuredBlockPyStmt<EIn, N>,
) -> StructuredBlockPyStmt<EOut, N>
where
    EOut: From<EIn>,
{
    match value {
        StructuredBlockPyStmt::Assign(assign) => StructuredBlockPyStmt::Assign(BlockPyAssign {
            target: assign.target,
            value: assign.value.into(),
        }),
        StructuredBlockPyStmt::Expr(expr) => StructuredBlockPyStmt::Expr(expr.into()),
        StructuredBlockPyStmt::Delete(delete) => StructuredBlockPyStmt::Delete(delete),
        StructuredBlockPyStmt::If(if_stmt) => StructuredBlockPyStmt::If(BlockPyIf {
            test: if_stmt.test.into(),
            body: convert_blockpy_fragment_expr(if_stmt.body),
            orelse: convert_blockpy_fragment_expr(if_stmt.orelse),
        }),
    }
}

#[derive(Debug, Clone)]
pub enum BlockPyStmt<E = CoreBlockPyExpr<LocatedName>, N = LocatedName> {
    Assign(BlockPyAssign<E, N>),
    Expr(E),
    Delete(BlockPyDelete<N>),
}

impl<E, N> From<BlockPyAssign<E, N>> for BlockPyStmt<E, N> {
    fn from(value: BlockPyAssign<E, N>) -> Self {
        Self::Assign(value)
    }
}

impl<N> From<CoreBlockPyExpr<N>> for BlockPyStmt<CoreBlockPyExpr<N>, N> {
    fn from(value: CoreBlockPyExpr<N>) -> Self {
        Self::Expr(value)
    }
}

impl<E, N> From<BlockPyDelete<N>> for BlockPyStmt<E, N> {
    fn from(value: BlockPyDelete<N>) -> Self {
        Self::Delete(value)
    }
}

impl<EIn, EOut, N> From<StructuredBlockPyStmt<EIn, N>> for BlockPyStmt<EOut, N>
where
    EOut: From<EIn>,
{
    fn from(value: StructuredBlockPyStmt<EIn, N>) -> Self {
        match value {
            StructuredBlockPyStmt::Assign(assign) => Self::Assign(BlockPyAssign {
                target: assign.target,
                value: assign.value.into(),
            }),
            StructuredBlockPyStmt::Expr(expr) => Self::Expr(expr.into()),
            StructuredBlockPyStmt::Delete(delete) => Self::Delete(delete),
            StructuredBlockPyStmt::If(_) => {
                panic!("structured BlockPy If reached BlockPyStmt conversion")
            }
        }
    }
}

impl<E: Clone + fmt::Debug, N: Clone + fmt::Debug> IntoStructuredBlockPyStmt<E, N>
    for BlockPyStmt<E, N>
{
    fn into_structured_stmt(self) -> StructuredBlockPyStmt<E, N> {
        match self {
            BlockPyStmt::Assign(assign) => StructuredBlockPyStmt::Assign(assign),
            BlockPyStmt::Expr(expr) => StructuredBlockPyStmt::Expr(expr),
            BlockPyStmt::Delete(delete) => StructuredBlockPyStmt::Delete(delete),
        }
    }
}

impl<E, N> BlockPyNormalizedStmt for BlockPyStmt<E, N> {
    fn assert_blockpy_normalized(&self) {}
}

impl<E: Clone + fmt::Debug, N: Clone + fmt::Debug> IntoStructuredBlockPyStmt<E, N>
    for StructuredBlockPyStmt<E, N>
{
    fn into_structured_stmt(self) -> StructuredBlockPyStmt<E, N> {
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
pub(crate) struct BlockPyIf<E = Expr, S = StructuredBlockPyStmt<E>, T = BlockPyTerm<E>> {
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

pub(crate) fn convert_blockpy_fragment_expr<EIn, EOut, N>(
    value: BlockPyCfgFragment<StructuredBlockPyStmt<EIn, N>, BlockPyTerm<EIn>>,
) -> BlockPyCfgFragment<StructuredBlockPyStmt<EOut, N>, BlockPyTerm<EOut>>
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
}

impl From<BlockPyLabel> for BlockPyEdge {
    fn from(value: BlockPyLabel) -> Self {
        Self::new(value)
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

pub(crate) trait BlockPyModuleVisitor<P>
where
    P: BlockPyPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
    P::Stmt: IntoStructuredBlockPyStmt<PassExpr<P>, PassName<P>>,
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

pub(crate) fn walk_module<V, P>(visitor: &mut V, module: &BlockPyModule<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
    P::Stmt: IntoStructuredBlockPyStmt<PassExpr<P>, PassName<P>>,
{
    for function in &module.callable_defs {
        visitor.visit_fn(function);
    }
}

pub(crate) fn walk_fn<V, P>(visitor: &mut V, func: &BlockPyFunction<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
    P::Stmt: IntoStructuredBlockPyStmt<PassExpr<P>, PassName<P>>,
{
    for block in &func.blocks {
        visitor.visit_block(block);
    }
}

pub(crate) fn walk_block<V, P>(visitor: &mut V, block: &PassBlock<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
    P::Stmt: IntoStructuredBlockPyStmt<PassExpr<P>, PassName<P>>,
{
    for stmt in &block.body {
        let stmt = stmt.clone().into_structured_stmt();
        visitor.visit_stmt(&stmt);
    }
    if let Some(exc_edge) = &block.exc_edge {
        visitor.visit_label(&exc_edge.target);
    }
    visitor.visit_term(&block.term);
}

pub(crate) fn walk_fragment<V, P>(
    visitor: &mut V,
    fragment: &BlockPyCfgFragment<StructuredPassStmt<P>, BlockPyTerm<PassExpr<P>>>,
) where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
    P::Stmt: IntoStructuredBlockPyStmt<PassExpr<P>, PassName<P>>,
{
    for stmt in &fragment.body {
        visitor.visit_stmt(stmt);
    }
    if let Some(term) = &fragment.term {
        visitor.visit_term(term);
    }
}

pub(crate) fn walk_stmt<V, P>(visitor: &mut V, stmt: &StructuredPassStmt<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
    P::Stmt: IntoStructuredBlockPyStmt<PassExpr<P>, PassName<P>>,
{
    match stmt {
        StructuredBlockPyStmt::Assign(assign) => visitor.visit_expr(&assign.value),
        StructuredBlockPyStmt::Expr(expr) => visitor.visit_expr(expr),
        StructuredBlockPyStmt::Delete(_) => {}
        StructuredBlockPyStmt::If(if_stmt) => {
            visitor.visit_expr(&if_stmt.test);
            visitor.visit_fragment(&if_stmt.body);
            visitor.visit_fragment(&if_stmt.orelse);
        }
    }
}

pub(crate) fn walk_label<V, P>(visitor: &mut V, label: &BlockPyLabel)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
    P::Stmt: IntoStructuredBlockPyStmt<PassExpr<P>, PassName<P>>,
{
    let _ = visitor;
    let _ = label;
}

pub(crate) fn walk_term<V, P>(visitor: &mut V, term: &BlockPyTerm<PassExpr<P>>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
    P::Stmt: IntoStructuredBlockPyStmt<PassExpr<P>, PassName<P>>,
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

pub(crate) fn walk_expr<V, P>(visitor: &mut V, expr: &PassExpr<P>)
where
    V: BlockPyModuleVisitor<P> + ?Sized,
    P: BlockPyPass,
    PassExpr<P>: MapExpr<PassExpr<P>>,
    P::Stmt: IntoStructuredBlockPyStmt<PassExpr<P>, PassName<P>>,
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

impl<N: BlockPyNameLike> ImplicitNoneExpr for CodegenBlockPyExpr<N> {
    fn implicit_none_expr() -> Self {
        Self::Name(implicit_none_name().into())
    }

    fn is_implicit_none_expr(expr: &Self) -> bool {
        matches!(expr, CodegenBlockPyExpr::Name(name) if name.id_str() == "__dp_NONE")
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
    pub(crate) fn visit_module(&self, visitor: &mut impl BlockPyModuleVisitor<PIn>)
    where
        PIn::Stmt: IntoStructuredBlockPyStmt<PassExpr<PIn>, PassName<PIn>>,
    {
        visitor.visit_module(self);
    }
}

pub fn count_ruff_blockpy_blocks(module: &BlockPyModule<RuffBlockPyPass>) -> usize {
    module
        .callable_defs
        .iter()
        .map(|function| count_structured_blockpy_blocks_in_list(&function.blocks))
        .sum()
}

fn count_structured_blockpy_blocks_in_list<S, E, N>(blocks: &[CfgBlock<S, BlockPyTerm<E>>]) -> usize
where
    S: IntoStructuredBlockPyStmt<E, N>,
    E: Clone + Into<Expr> + std::fmt::Debug,
    N: BlockPyNameLike,
{
    blocks
        .iter()
        .map(|block| {
            1 + count_structured_blockpy_blocks_in_stmts(&block.body)
                + count_structured_blockpy_blocks_in_term(&block.term)
        })
        .sum()
}

fn count_structured_blockpy_blocks_in_stmts<S, E, N>(stmts: &[S]) -> usize
where
    S: IntoStructuredBlockPyStmt<E, N>,
    E: Clone + Into<Expr> + std::fmt::Debug,
    N: BlockPyNameLike,
{
    stmts
        .iter()
        .map(|stmt| match stmt.clone().into_structured_stmt() {
            StructuredBlockPyStmt::If(if_stmt) => {
                count_structured_blockpy_blocks_in_fragment(&if_stmt.body)
                    + count_structured_blockpy_blocks_in_fragment(&if_stmt.orelse)
            }
            StructuredBlockPyStmt::Assign(_)
            | StructuredBlockPyStmt::Expr(_)
            | StructuredBlockPyStmt::Delete(_) => 0,
        })
        .sum()
}

fn count_structured_blockpy_blocks_in_fragment<E, N>(
    fragment: &BlockPyCfgFragment<StructuredBlockPyStmt<E, N>, BlockPyTerm<E>>,
) -> usize
where
    E: Clone + Into<Expr> + std::fmt::Debug,
    N: BlockPyNameLike,
{
    count_structured_blockpy_blocks_in_stmts(&fragment.body)
        + fragment
            .term
            .as_ref()
            .map_or(0, count_structured_blockpy_blocks_in_term)
}

fn count_structured_blockpy_blocks_in_term<E>(term: &BlockPyTerm<E>) -> usize {
    match term {
        BlockPyTerm::IfTerm(_) => 0,
        BlockPyTerm::Jump(_)
        | BlockPyTerm::BranchTable(_)
        | BlockPyTerm::Raise(_)
        | BlockPyTerm::Return(_) => 0,
    }
}

#[cfg(test)]
mod test;

impl BlockPyLabel {
    pub fn index(self) -> usize {
        self.index as usize
    }
}

impl fmt::Display for BlockPyLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bb{}", self.index)
    }
}
