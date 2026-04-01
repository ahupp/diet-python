use crate::block_py::{
    core_operation_expr, BlockPyModule, BlockPyModuleMap, CoreBlockPyLiteral, HasMeta,
    LocatedCodegenBlockPyExpr, LocatedCoreBlockPyExpr, MakeString, OperationDetail, WithMeta,
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
        match expr {
            LocatedCoreBlockPyExpr::Literal(CoreBlockPyLiteral::StringLiteral(node)) => {
                let meta = node.meta();
                core_operation_expr(
                    OperationDetail::from(MakeString::new(node.value.into_bytes())).with_meta(meta),
                )
            }
            _ => self.map_nested_expr(expr),
        }
    }
}

#[cfg(test)]
mod test;
