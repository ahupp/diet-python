use crate::block_py::{
    core_operation_expr, BlockPyFunction, BlockPyLiteral, BlockPyModule, BlockPyModuleMap,
    CodegenBlockPyExpr, HasMeta, InstrExprNode, LiteralValue, Load, LocatedCoreBlockPyExpr,
    LocatedName, MapExpr, NameLocation, WithMeta,
};
use crate::passes::{CodegenBlockPyPass, ResolvedStorageBlockPyPass};
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
        let meta = literal.meta();
        module_constants.push(LocatedCoreBlockPyExpr::Literal(
            LiteralValue::new(literal).with_meta(meta),
        ));
        index
    }
}

impl MapExpr<LocatedCoreBlockPyExpr, CodegenBlockPyExpr> for CodegenExprNormalizer {
    fn map_expr(&self, expr: LocatedCoreBlockPyExpr) -> CodegenBlockPyExpr {
        match expr {
            LocatedCoreBlockPyExpr::Literal(literal) => {
                let meta = literal.meta();
                let constant_index = self.push_module_constant(literal.into_literal());
                core_operation_expr(
                    Load::new(LocatedName {
                        id: format!("__dp_constant_{constant_index}").into(),
                        location: NameLocation::Constant(constant_index),
                    })
                    .with_meta(meta),
                )
            }
            LocatedCoreBlockPyExpr::BinOp(node) => {
                node.map_children(&mut |child| self.map_expr(child)).into()
            }
            LocatedCoreBlockPyExpr::UnaryOp(node) => {
                node.map_children(&mut |child| self.map_expr(child)).into()
            }
            LocatedCoreBlockPyExpr::Call(node) => {
                node.map_children(&mut |child| self.map_expr(child)).into()
            }
            LocatedCoreBlockPyExpr::GetAttr(node) => {
                node.map_children(&mut |child| self.map_expr(child)).into()
            }
            LocatedCoreBlockPyExpr::SetAttr(node) => {
                node.map_children(&mut |child| self.map_expr(child)).into()
            }
            LocatedCoreBlockPyExpr::GetItem(node) => {
                node.map_children(&mut |child| self.map_expr(child)).into()
            }
            LocatedCoreBlockPyExpr::SetItem(node) => {
                node.map_children(&mut |child| self.map_expr(child)).into()
            }
            LocatedCoreBlockPyExpr::DelItem(node) => {
                node.map_children(&mut |child| self.map_expr(child)).into()
            }
            LocatedCoreBlockPyExpr::Load(node) => {
                node.map_children(&mut |child| self.map_expr(child)).into()
            }
            LocatedCoreBlockPyExpr::Store(node) => {
                node.map_children(&mut |child| self.map_expr(child)).into()
            }
            LocatedCoreBlockPyExpr::Del(node) => {
                node.map_children(&mut |child| self.map_expr(child)).into()
            }
            LocatedCoreBlockPyExpr::MakeCell(node) => {
                node.map_children(&mut |child| self.map_expr(child)).into()
            }
            LocatedCoreBlockPyExpr::CellRefForName(node) => node.into(),
            LocatedCoreBlockPyExpr::CellRef(node) => node.into(),
            LocatedCoreBlockPyExpr::MakeFunction(node) => {
                node.map_children(&mut |child| self.map_expr(child)).into()
            }
        }
    }
}

impl BlockPyModuleMap<ResolvedStorageBlockPyPass, CodegenBlockPyPass> for CodegenExprNormalizer {}

#[cfg(test)]
mod test;
