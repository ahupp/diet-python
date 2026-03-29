use crate::block_py::{
    core_operation_expr, BlockPyModule, BlockPyModuleMap, CoreBlockPyLiteral,
    LocatedCodegenBlockPyExpr, LocatedCoreBlockPyExpr, MakeString,
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
                core_operation_expr(crate::block_py::Operation::MakeString(MakeString {
                    node_index: node.node_index.clone(),
                    range: node.range,
                    arg0: node.value.into_bytes(),
                }))
            }
            _ => self.map_nested_expr(expr),
        }
    }
}

#[cfg(test)]
mod test;
