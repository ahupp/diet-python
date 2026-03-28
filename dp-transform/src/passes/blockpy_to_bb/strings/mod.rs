use crate::block_py::{
    core_operation_expr, BlockPyModule, BlockPyModuleMap, CodegenBlockPyLiteral,
    CoreBlockPyLiteral, CoreBytesLiteral, LocatedCodegenBlockPyExpr, LocatedCoreBlockPyExpr,
    MakeString,
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
                    arg0: bytes_literal_expr_with_meta(
                        node.value.as_bytes(),
                        (node.node_index, node.range),
                    ),
                }))
            }
            _ => self.map_nested_expr(expr),
        }
    }
}

fn bytes_literal_expr_with_meta(
    bytes: &[u8],
    (node_index, range): (ruff_python_ast::AtomicNodeIndex, ruff_text_size::TextRange),
) -> LocatedCodegenBlockPyExpr {
    LocatedCodegenBlockPyExpr::Literal(CodegenBlockPyLiteral::BytesLiteral(CoreBytesLiteral {
        range,
        node_index,
        value: bytes.to_vec(),
    }))
}

#[cfg(test)]
mod test;
