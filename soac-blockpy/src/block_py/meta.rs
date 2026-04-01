use super::{
    BlockPyNameLike, CodegenBlockPyExpr, CodegenBlockPyLiteral, CoreBlockPyAwait, CoreBlockPyExpr,
    CoreBlockPyExprWithAwaitAndYield, CoreBlockPyExprWithYield, CoreBlockPyLiteral,
    CoreBlockPyYield, CoreBlockPyYieldFrom, CoreBytesLiteral, CoreNumberLiteral, CoreStringLiteral,
    LocatedName, RuffExpr, UnresolvedName,
};
use ruff_python_ast::{self as ast, HasNodeIndex};
use ruff_text_size::{Ranged, TextRange};

#[derive(Debug, Clone, Default)]
pub struct Meta {
    pub node_index: ast::AtomicNodeIndex,
    pub range: TextRange,
}

impl Meta {
    pub fn new(node_index: ast::AtomicNodeIndex, range: TextRange) -> Self {
        Self { node_index, range }
    }

    pub fn synthetic() -> Self {
        Self::default()
    }
}

pub trait HasMeta {
    fn meta(&self) -> Meta;
}

pub trait WithMeta: Sized {
    fn with_meta(self, meta: Meta) -> Self;

    fn with_source<T: HasMeta>(self, source: &T) -> Self {
        self.with_meta(source.meta())
    }
}

impl<T> HasMeta for T
where
    T: HasNodeIndex + Ranged,
{
    fn meta(&self) -> Meta {
        Meta::new(self.node_index().clone(), self.range())
    }
}

impl HasMeta for RuffExpr {
    fn meta(&self) -> Meta {
        self.0.meta()
    }
}

impl HasMeta for LocatedName {
    fn meta(&self) -> Meta {
        Meta::new(self.node_index.clone(), self.range)
    }
}

impl HasMeta for UnresolvedName {
    fn meta(&self) -> Meta {
        Meta::new(self.node_index(), self.range())
    }
}

impl HasMeta for CoreBlockPyExprWithAwaitAndYield {
    fn meta(&self) -> Meta {
        match self {
            Self::Name(name) => name.meta(),
            Self::Literal(literal) => literal.meta(),
            Self::Op(operation) => operation.meta(),
            Self::Await(await_expr) => await_expr.meta(),
            Self::Yield(yield_expr) => yield_expr.meta(),
            Self::YieldFrom(yield_from_expr) => yield_from_expr.meta(),
        }
    }
}

impl HasMeta for CoreBlockPyExprWithYield {
    fn meta(&self) -> Meta {
        match self {
            Self::Name(name) => name.meta(),
            Self::Literal(literal) => literal.meta(),
            Self::Op(operation) => operation.meta(),
            Self::Yield(yield_expr) => yield_expr.meta(),
            Self::YieldFrom(yield_from_expr) => yield_from_expr.meta(),
        }
    }
}

impl<N: BlockPyNameLike> HasMeta for CoreBlockPyExpr<N> {
    fn meta(&self) -> Meta {
        match self {
            Self::Name(name) => Meta::new(name.node_index(), name.range()),
            Self::Literal(literal) => literal.meta(),
            Self::Op(operation) => operation.meta(),
        }
    }
}

impl HasMeta for CodegenBlockPyExpr {
    fn meta(&self) -> Meta {
        match self {
            Self::Name(name) => Meta::new(name.node_index(), name.range()),
            Self::Literal(literal) => literal.meta(),
            Self::Op(operation) => operation.meta(),
        }
    }
}

impl HasMeta for CoreBlockPyLiteral {
    fn meta(&self) -> Meta {
        match self {
            Self::StringLiteral(literal) => literal.meta(),
            Self::BytesLiteral(literal) => literal.meta(),
            Self::NumberLiteral(literal) => literal.meta(),
        }
    }
}

impl HasMeta for CodegenBlockPyLiteral {
    fn meta(&self) -> Meta {
        match self {
            Self::StringLiteral(literal) => literal.meta(),
            Self::BytesLiteral(literal) => literal.meta(),
            Self::NumberLiteral(literal) => literal.meta(),
        }
    }
}

impl HasMeta for CoreStringLiteral {
    fn meta(&self) -> Meta {
        Meta::new(self.node_index.clone(), self.range)
    }
}

impl HasMeta for CoreBytesLiteral {
    fn meta(&self) -> Meta {
        Meta::new(self.node_index.clone(), self.range)
    }
}

impl HasMeta for CoreNumberLiteral {
    fn meta(&self) -> Meta {
        Meta::new(self.node_index.clone(), self.range)
    }
}

impl<E> HasMeta for CoreBlockPyAwait<E> {
    fn meta(&self) -> Meta {
        Meta::new(self.node_index.clone(), self.range)
    }
}

impl<E> WithMeta for CoreBlockPyAwait<E> {
    fn with_meta(mut self, meta: Meta) -> Self {
        self.node_index = meta.node_index;
        self.range = meta.range;
        self
    }
}

impl<E> HasMeta for CoreBlockPyYield<E> {
    fn meta(&self) -> Meta {
        Meta::new(self.node_index.clone(), self.range)
    }
}

impl<E> WithMeta for CoreBlockPyYield<E> {
    fn with_meta(mut self, meta: Meta) -> Self {
        self.node_index = meta.node_index;
        self.range = meta.range;
        self
    }
}

impl<E> HasMeta for CoreBlockPyYieldFrom<E> {
    fn meta(&self) -> Meta {
        Meta::new(self.node_index.clone(), self.range)
    }
}

impl<E> WithMeta for CoreBlockPyYieldFrom<E> {
    fn with_meta(mut self, meta: Meta) -> Self {
        self.node_index = meta.node_index;
        self.range = meta.range;
        self
    }
}
