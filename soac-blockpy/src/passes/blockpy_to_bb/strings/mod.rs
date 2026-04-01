use crate::block_py::{
    BlockPyModule, BlockPyModuleMap, LocatedCodegenBlockPyExpr, LocatedCoreBlockPyExpr,
};
use crate::passes::{CodegenBlockPyPass, ResolvedStorageBlockPyPass};

pub fn normalize_bb_module_strings(
    module: &BlockPyModule<ResolvedStorageBlockPyPass>,
) -> BlockPyModule<CodegenBlockPyPass> {
    module.clone().map_module(&CodegenExprNormalizer)
}

struct CodegenExprNormalizer;

impl BlockPyModuleMap<ResolvedStorageBlockPyPass, CodegenBlockPyPass> for CodegenExprNormalizer {
    fn map_expr(&self, expr: LocatedCoreBlockPyExpr) -> LocatedCodegenBlockPyExpr {
        self.map_nested_expr(expr)
    }
}

#[cfg(test)]
mod test;
