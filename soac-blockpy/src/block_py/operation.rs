use super::operation_macro::define_operation;
use super::{
    BlockPyNameLike, CellLocation, CoreBlockPyCallArg, CoreBlockPyKeywordArg, FunctionId,
    FunctionKind, HasMeta, Instr, InstrExprNode, InstrName, MapExpr, Meta, TryMapExpr, Walkable,
    WithMeta,
};
use std::fmt;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum BinOpKind {
    Add,
    Sub,
    Mul,
    MatMul,
    TrueDiv,
    FloorDiv,
    Mod,
    Pow,
    LShift,
    RShift,
    Or,
    Xor,
    And,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Contains,
    Is,
    InplaceAdd,
    InplaceSub,
    InplaceMul,
    InplaceMatMul,
    InplaceTrueDiv,
    InplaceFloorDiv,
    InplaceMod,
    InplacePow,
    InplaceLShift,
    InplaceRShift,
    InplaceOr,
    InplaceXor,
    InplaceAnd,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum UnaryOpKind {
    Pos,
    Neg,
    Invert,
    Not,
    Truth,
}

define_operation! {
    pub struct BinOp<E> {
        kind: BinOpKind,
        left: Box<E>,
        right: Box<E>,
    }
}

define_operation! {
    pub struct UnaryOp<E> {
        kind: UnaryOpKind,
        operand: Box<E>,
    }
}

#[derive(Clone)]
pub struct Call<E> {
    _meta: Meta,
    pub func: Box<E>,
    pub args: Vec<CoreBlockPyCallArg<E>>,
    pub keywords: Vec<CoreBlockPyKeywordArg<E>>,
}

impl<E: fmt::Debug> fmt::Debug for Call<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}(", self.func)?;
        let mut first = true;
        for arg in &self.args {
            if !first {
                write!(f, ", ")?;
            }
            first = false;
            match arg {
                CoreBlockPyCallArg::Positional(expr) => write!(f, "{expr:?}")?,
                CoreBlockPyCallArg::Starred(expr) => write!(f, "*{expr:?}")?,
            }
        }
        for keyword in &self.keywords {
            if !first {
                write!(f, ", ")?;
            }
            first = false;
            match keyword {
                CoreBlockPyKeywordArg::Named { arg, value } => write!(f, "{arg}={value:?}")?,
                CoreBlockPyKeywordArg::Starred(value) => write!(f, "**{value:?}")?,
            }
        }
        write!(f, ")")
    }
}

impl<E> Call<E> {
    pub fn new(
        func: impl Into<Box<E>>,
        args: impl Into<Vec<CoreBlockPyCallArg<E>>>,
        keywords: impl Into<Vec<CoreBlockPyKeywordArg<E>>>,
    ) -> Self {
        Self {
            _meta: Meta::default(),
            func: func.into(),
            args: args.into(),
            keywords: keywords.into(),
        }
    }
}

impl<E> HasMeta for Call<E> {
    fn meta(&self) -> Meta {
        self._meta.clone()
    }
}

impl<E> WithMeta for Call<E> {
    fn with_meta(mut self, meta: Meta) -> Self {
        self._meta = meta;
        self
    }
}

impl<E: Instr> Walkable<E> for Call<E> {
    fn map_walk(self, f: &mut impl FnMut(E) -> E) -> Self {
        Call {
            _meta: self._meta,
            func: Box::new(f(*self.func)),
            args: self
                .args
                .into_iter()
                .map(|arg| arg.map_expr(&mut *f))
                .collect(),
            keywords: self
                .keywords
                .into_iter()
                .map(|keyword| keyword.map_expr(&mut *f))
                .collect(),
        }
    }

    fn walk_mut(&mut self, f: &mut impl FnMut(&mut E)) {
        f(&mut self.func);
        for arg in &mut self.args {
            f(arg.expr_mut());
        }
        for keyword in &mut self.keywords {
            f(keyword.expr_mut());
        }
    }

    fn walk(&self, f: &mut impl FnMut(&E)) {
        f(&self.func);
        for arg in &self.args {
            f(arg.expr());
        }
        for keyword in &self.keywords {
            f(keyword.expr());
        }
    }
}

impl<E: Instr> InstrExprNode<E> for Call<E> {
    type Mapped<T: Instr> = Call<T>;

    fn map_typed_children<T, M>(self, map: &mut M) -> Self::Mapped<T>
    where
        T: Instr,
        M: MapExpr<E, T>,
    {
        Call {
            _meta: self._meta,
            func: Box::new(map.map_expr(*self.func)),
            args: self
                .args
                .into_iter()
                .map(|arg| arg.map_expr(|expr| map.map_expr(expr)))
                .collect(),
            keywords: self
                .keywords
                .into_iter()
                .map(|keyword| keyword.map_expr(|expr| map.map_expr(expr)))
                .collect(),
        }
    }

    fn try_map_typed_children<T, Error, M>(self, map: &mut M) -> Result<Self::Mapped<T>, Error>
    where
        T: Instr,
        M: TryMapExpr<E, T, Error>,
    {
        Ok(Call {
            _meta: self._meta,
            func: Box::new(map.try_map_expr(*self.func)?),
            args: self
                .args
                .into_iter()
                .map(|arg| arg.try_map_expr(|expr| map.try_map_expr(expr)))
                .collect::<Result<Vec<_>, _>>()?,
            keywords: self
                .keywords
                .into_iter()
                .map(|keyword| keyword.try_map_expr(|expr| map.try_map_expr(expr)))
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

define_operation! {
    pub struct GetAttr<E> {
        value: Box<E>,
        attr: Box<E>,
    }
}

define_operation! {
    pub struct SetAttr<E> {
        value: Box<E>,
        attr: Box<E>,
        replacement: Box<E>,
    }
}

define_operation! {
    pub struct GetItem<E> {
        value: Box<E>,
        index: Box<E>,
    }
}

define_operation! {
    pub struct SetItem<E> {
        value: Box<E>,
        index: Box<E>,
        replacement: Box<E>,
    }
}

define_operation! {
    pub struct DelItem<E> {
        value: Box<E>,
        index: Box<E>,
    }
}

#[derive(Clone)]
pub struct Load<I: Instr> {
    _meta: Meta,
    pub name: InstrName<I>,
}

impl<I: Instr> fmt::Debug for Load<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name.pretty_id())
    }
}

impl<I: Instr> Load<I> {
    pub fn new(name: impl Into<InstrName<I>>) -> Self {
        Self {
            _meta: Meta::default(),
            name: name.into(),
        }
    }
}

impl<I: Instr> HasMeta for Load<I> {
    fn meta(&self) -> Meta {
        self._meta.clone()
    }
}

impl<I: Instr> WithMeta for Load<I> {
    fn with_meta(mut self, meta: Meta) -> Self {
        self._meta = meta;
        self
    }
}

impl<I: Instr> Walkable<I> for Load<I> {
    fn map_walk(self, _f: &mut impl FnMut(I) -> I) -> Self {
        self
    }

    fn walk_mut(&mut self, _f: &mut impl FnMut(&mut I)) {}

    fn walk(&self, _f: &mut impl FnMut(&I)) {}
}

impl<I: Instr> InstrExprNode<I> for Load<I> {
    type Mapped<T: Instr> = Load<T>;

    fn map_typed_children<T, M>(self, map: &mut M) -> Self::Mapped<T>
    where
        T: Instr,
        M: MapExpr<I, T>,
    {
        Load {
            _meta: self._meta,
            name: map.map_name(self.name),
        }
    }

    fn try_map_typed_children<T, Error, M>(self, map: &mut M) -> Result<Self::Mapped<T>, Error>
    where
        T: Instr,
        M: TryMapExpr<I, T, Error>,
    {
        Ok(Load {
            _meta: self._meta,
            name: map.try_map_name(self.name)?,
        })
    }
}

#[derive(Clone)]
pub struct Store<I: Instr> {
    _meta: Meta,
    pub name: InstrName<I>,
    pub value: Box<I>,
}

impl<I: Instr> fmt::Debug for Store<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.name.pretty_id() == self.name.id_str() {
            write!(f, "StoreName({:?}, {:?})", self.name.id_str(), self.value)
        } else {
            write!(
                f,
                "StoreLocation({}, {:?})",
                self.name.pretty_id(),
                self.value
            )
        }
    }
}

impl<I: Instr> Store<I> {
    pub fn new(name: impl Into<InstrName<I>>, value: impl Into<Box<I>>) -> Self {
        Self {
            _meta: Meta::default(),
            name: name.into(),
            value: value.into(),
        }
    }
}

impl<I: Instr> HasMeta for Store<I> {
    fn meta(&self) -> Meta {
        self._meta.clone()
    }
}

impl<I: Instr> WithMeta for Store<I> {
    fn with_meta(mut self, meta: Meta) -> Self {
        self._meta = meta;
        self
    }
}

impl<I: Instr> Walkable<I> for Store<I> {
    fn map_walk(self, f: &mut impl FnMut(I) -> I) -> Self {
        Store {
            _meta: self._meta,
            name: self.name,
            value: Box::new(f(*self.value)),
        }
    }

    fn walk_mut(&mut self, f: &mut impl FnMut(&mut I)) {
        f(&mut self.value);
    }

    fn walk(&self, f: &mut impl FnMut(&I)) {
        f(&self.value);
    }
}

impl<I: Instr> InstrExprNode<I> for Store<I> {
    type Mapped<T: Instr> = Store<T>;

    fn map_typed_children<T, M>(self, map: &mut M) -> Self::Mapped<T>
    where
        T: Instr,
        M: MapExpr<I, T>,
    {
        Store {
            _meta: self._meta,
            name: map.map_name(self.name),
            value: Box::new(map.map_expr(*self.value)),
        }
    }

    fn try_map_typed_children<T, Error, M>(self, map: &mut M) -> Result<Self::Mapped<T>, Error>
    where
        T: Instr,
        M: TryMapExpr<I, T, Error>,
    {
        Ok(Store {
            _meta: self._meta,
            name: map.try_map_name(self.name)?,
            value: Box::new(map.try_map_expr(*self.value)?),
        })
    }
}

#[derive(Clone)]
pub struct Del<I: Instr> {
    _meta: Meta,
    pub name: InstrName<I>,
    pub quietly: bool,
}

impl<I: Instr> fmt::Debug for Del<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Del")
            .field("name", &self.name.pretty_id())
            .field("quietly", &self.quietly)
            .finish()
    }
}

impl<I: Instr> Del<I> {
    pub fn new(name: impl Into<InstrName<I>>, quietly: bool) -> Self {
        Self {
            _meta: Meta::default(),
            name: name.into(),
            quietly,
        }
    }
}

impl<I: Instr> HasMeta for Del<I> {
    fn meta(&self) -> Meta {
        self._meta.clone()
    }
}

impl<I: Instr> WithMeta for Del<I> {
    fn with_meta(mut self, meta: Meta) -> Self {
        self._meta = meta;
        self
    }
}

impl<I: Instr> Walkable<I> for Del<I> {
    fn map_walk(self, _f: &mut impl FnMut(I) -> I) -> Self {
        self
    }

    fn walk_mut(&mut self, _f: &mut impl FnMut(&mut I)) {}

    fn walk(&self, _f: &mut impl FnMut(&I)) {}
}

impl<I: Instr> InstrExprNode<I> for Del<I> {
    type Mapped<T: Instr> = Del<T>;

    fn map_typed_children<T, M>(self, map: &mut M) -> Self::Mapped<T>
    where
        T: Instr,
        M: MapExpr<I, T>,
    {
        Del {
            _meta: self._meta,
            name: map.map_name(self.name),
            quietly: self.quietly,
        }
    }

    fn try_map_typed_children<T, Error, M>(self, map: &mut M) -> Result<Self::Mapped<T>, Error>
    where
        T: Instr,
        M: TryMapExpr<I, T, Error>,
    {
        Ok(Del {
            _meta: self._meta,
            name: map.try_map_name(self.name)?,
            quietly: self.quietly,
        })
    }
}

define_operation! {
    pub struct MakeCell<E> {
        initial_value: Box<E>,
    }
}

define_operation! {
    pub struct CellRefForName {
        logical_name: String,
    }
}

define_operation! {
    pub struct CellRef {
        location: CellLocation,
    }
}

define_operation! {
    pub struct MakeFunction<E> {
        function_id: FunctionId,
        kind: FunctionKind,
        param_defaults: Box<E>,
        annotate_fn: Box<E>,
    }
}

define_operation! {
    pub struct Await<E> {
        value: Box<E>,
    }
}

define_operation! {
    pub struct Yield<E> {
        value: Box<E>,
    }
}

define_operation! {
    pub struct YieldFrom<E> {
        value: Box<E>,
    }
}
