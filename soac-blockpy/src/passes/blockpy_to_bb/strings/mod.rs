use crate::block_py::{
    core_operation_expr, BlockPyFunction, BlockPyLiteral, BlockPyModule, BlockPyModuleMap,
    CodegenBlockPyExpr, HasMeta, Load, LocatedCodegenBlockPyExpr, LocatedCoreBlockPyExpr,
    LocatedName, MapExpr, NameLocation, WithMeta,
};
use crate::passes::{CodegenBlockPyPass, ResolvedStorageBlockPyPass};
use ruff_python_ast as ast;
use std::cell::RefCell;

pub fn normalize_bb_module_strings(
    module: &BlockPyModule<ResolvedStorageBlockPyPass>,
) -> BlockPyModule<CodegenBlockPyPass> {
    let normalizer = CodegenExprNormalizer::default();
    let module = module.clone();
    let mut module_constants = module.module_constants;
    module_constants.extend(normalizer.module_constants.borrow().iter().cloned());
    BlockPyModule {
        callable_defs: module
            .callable_defs
            .into_iter()
            .map(|function| normalizer.map_fn(function))
            .collect::<Vec<BlockPyFunction<CodegenBlockPyPass>>>(),
        module_constants,
    }
}

#[derive(Default)]
struct CodegenExprNormalizer {
    module_constants: RefCell<Vec<LocatedCoreBlockPyExpr>>,
}

impl CodegenExprNormalizer {
    fn push_module_constant(&self, literal: BlockPyLiteral) -> u32 {
        let mut module_constants = self.module_constants.borrow_mut();
        let index =
            u32::try_from(module_constants.len()).expect("module constant count should fit in u32");
        module_constants.push(LocatedCoreBlockPyExpr::Literal(literal));
        index
    }
}

impl BlockPyModuleMap<ResolvedStorageBlockPyPass, CodegenBlockPyPass> for CodegenExprNormalizer {
    fn map_expr(&self, expr: LocatedCoreBlockPyExpr) -> LocatedCodegenBlockPyExpr {
        match expr {
            LocatedCoreBlockPyExpr::Literal(literal) => {
                let meta = literal.meta();
                let constant_index = self.push_module_constant(literal);
                core_operation_expr(
                    Load::new(LocatedName {
                        id: format!("__dp_constant_{constant_index}").into(),
                        ctx: ast::ExprContext::Load,
                        range: meta.range,
                        node_index: meta.node_index.clone(),
                        location: NameLocation::Constant(constant_index),
                    })
                    .with_meta(meta),
                )
            }
            _ => self.map_nested_expr(expr),
        }
    }
}

#[cfg(test)]
mod test;
