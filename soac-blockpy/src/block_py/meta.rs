use super::{
    BlockPyNameLike, CodegenBlockPyLiteral, CoreBlockPyAwait, CoreBlockPyExpr,
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

impl WithMeta for LocatedName {
    fn with_meta(mut self, meta: Meta) -> Self {
        self.node_index = meta.node_index;
        self.range = meta.range;
        self
    }
}

impl WithMeta for UnresolvedName {
    fn with_meta(self, meta: Meta) -> Self {
        match self {
            Self::ExprName(mut name) => {
                name.node_index = meta.node_index;
                name.range = meta.range;
                Self::ExprName(name)
            }
            Self::RuntimeName(literal) => Self::RuntimeName(literal.with_meta(meta)),
        }
    }
}

impl WithMeta for CoreStringLiteral {
    fn with_meta(mut self, meta: Meta) -> Self {
        self.node_index = meta.node_index;
        self.range = meta.range;
        self
    }
}

impl WithMeta for CoreBytesLiteral {
    fn with_meta(mut self, meta: Meta) -> Self {
        self.node_index = meta.node_index;
        self.range = meta.range;
        self
    }
}

impl WithMeta for CoreNumberLiteral {
    fn with_meta(mut self, meta: Meta) -> Self {
        self.node_index = meta.node_index;
        self.range = meta.range;
        self
    }
}

impl WithMeta for CoreBlockPyLiteral {
    fn with_meta(self, meta: Meta) -> Self {
        match self {
            Self::StringLiteral(literal) => Self::StringLiteral(literal.with_meta(meta)),
            Self::BytesLiteral(literal) => Self::BytesLiteral(literal.with_meta(meta)),
            Self::NumberLiteral(literal) => Self::NumberLiteral(literal.with_meta(meta)),
        }
    }
}

impl WithMeta for CodegenBlockPyLiteral {
    fn with_meta(self, meta: Meta) -> Self {
        match self {
            Self::StringLiteral(literal) => Self::StringLiteral(literal.with_meta(meta)),
            Self::BytesLiteral(literal) => Self::BytesLiteral(literal.with_meta(meta)),
            Self::NumberLiteral(literal) => Self::NumberLiteral(literal.with_meta(meta)),
        }
    }
}
