use crate::basic_block::block_py::{is_internal_entry_livein, BlockPyPass, BlockPyStmt};

pub use super::block_py::BbBlockMeta;
use super::block_py::{BbBlockPyPass, BlockPyFunction, PassBlock};

pub type BbStmt = BlockPyStmt<<BbBlockPyPass as BlockPyPass>::Expr>;
pub type BbBlock = PassBlock<BbBlockPyPass>;

impl BlockPyFunction<BbBlockPyPass> {
    pub fn entry_liveins(&self) -> Vec<String> {
        if self.blocks.is_empty() {
            return Vec::new();
        }
        self.entry_block()
            .meta
            .params
            .iter()
            .filter(|name| !is_internal_entry_livein(name))
            .cloned()
            .collect()
    }
}
