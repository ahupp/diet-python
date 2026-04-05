use super::RuffExpr;
use ruff_python_ast::{self as ast, HasNodeIndex};
use ruff_text_size::{Ranged, TextRange};
use std::num::NonZeroU32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InstrId(pub NonZeroU32);

impl InstrId {
    pub fn new(value: u32) -> Option<Self> {
        NonZeroU32::new(value).map(Self)
    }

    pub fn get(self) -> u32 {
        self.0.get()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Meta {
    pub node_index: ast::AtomicNodeIndex,
    pub instr_id: Option<InstrId>,
    pub range: TextRange,
}

impl Meta {
    pub fn new(node_index: ast::AtomicNodeIndex, range: TextRange) -> Self {
        Self {
            node_index,
            instr_id: None,
            range,
        }
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
