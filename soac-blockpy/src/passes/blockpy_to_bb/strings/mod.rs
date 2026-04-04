use crate::block_py::{
    map_fn, BlockPyFunction, BlockPyModule, CodegenBlockPyExpr, HasMeta, InstrExprNode,
    LiteralValue, Load, LocatedCoreBlockPyExpr, LocatedName, MapExpr, NameLocation, WithMeta,
};
use crate::passes::{CodegenBlockPyPass, ResolvedStorageBlockPyPass};

pub fn normalize_bb_module_strings(
    module: &BlockPyModule<ResolvedStorageBlockPyPass>,
) -> BlockPyModule<CodegenBlockPyPass> {
    let mut normalizer = CodegenExprNormalizer::default();
    let module = module.clone();
    let mut module_constants = module.module_constants;
    let callable_defs = module
        .callable_defs
        .into_iter()
        .map(|function| map_fn(&mut normalizer, function))
        .collect::<Vec<BlockPyFunction<CodegenBlockPyPass>>>();
    module_constants.extend(normalizer.module_constants);
    BlockPyModule {
        module_name_gen: module.module_name_gen,
        callable_defs,
        module_constants,
        counter_defs: module.counter_defs,
    }
}

#[derive(Default)]
struct CodegenExprNormalizer {
    module_constants: Vec<LocatedCoreBlockPyExpr>,
}

impl CodegenExprNormalizer {
    fn push_module_constant(&mut self, literal: LiteralValue) -> u32 {
        let index = u32::try_from(self.module_constants.len())
            .expect("module constant count should fit in u32");
        self.module_constants
            .push(LocatedCoreBlockPyExpr::Literal(literal));
        index
    }
}

impl MapExpr<LocatedCoreBlockPyExpr, CodegenBlockPyExpr> for CodegenExprNormalizer {
    fn map_expr(&mut self, expr: LocatedCoreBlockPyExpr) -> CodegenBlockPyExpr {
        match expr {
            LocatedCoreBlockPyExpr::Literal(literal) => {
                let meta = literal.meta();
                let constant_index = self.push_module_constant(literal);
                Load::new(LocatedName {
                    id: format!("__dp_constant_{constant_index}").into(),
                    location: NameLocation::Constant(constant_index),
                })
                .with_meta(meta)
                .into()
            }
            LocatedCoreBlockPyExpr::BinOp(node) => node.map_typed_children(self).into(),
            LocatedCoreBlockPyExpr::UnaryOp(node) => node.map_typed_children(self).into(),
            LocatedCoreBlockPyExpr::Call(node) => node.map_typed_children(self).into(),
            LocatedCoreBlockPyExpr::GetAttr(node) => node.map_typed_children(self).into(),
            LocatedCoreBlockPyExpr::SetAttr(node) => node.map_typed_children(self).into(),
            LocatedCoreBlockPyExpr::GetItem(node) => node.map_typed_children(self).into(),
            LocatedCoreBlockPyExpr::SetItem(node) => node.map_typed_children(self).into(),
            LocatedCoreBlockPyExpr::DelItem(node) => node.map_typed_children(self).into(),
            LocatedCoreBlockPyExpr::Load(node) => node.map_typed_children(self).into(),
            LocatedCoreBlockPyExpr::Store(node) => node.map_typed_children(self).into(),
            LocatedCoreBlockPyExpr::Del(node) => node.map_typed_children(self).into(),
            LocatedCoreBlockPyExpr::MakeCell(node) => node.map_typed_children(self).into(),
            LocatedCoreBlockPyExpr::CellRefForName(node) => {
                panic!(
                    "cell_ref should lower to a resolved cell ref before codegen, got {:?}",
                    node.logical_name
                );
            }
            LocatedCoreBlockPyExpr::CellRef(node) => node.into(),
            LocatedCoreBlockPyExpr::MakeFunction(node) => node.map_typed_children(self).into(),
        }
    }

    fn map_name(&mut self, name: LocatedName) -> LocatedName {
        name
    }
}

#[cfg(test)]
mod test;
