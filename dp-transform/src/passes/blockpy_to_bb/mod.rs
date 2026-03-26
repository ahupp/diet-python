mod exception_pass;
mod strings;

use super::blockpy_generators::lower_generator_like_function;
use super::core_eval_order::make_eval_order_explicit_in_core_callable_def_without_await;
use super::ruff_to_blockpy::{
    lower_structured_located_blocks_to_bb_blocks,
    populate_exception_edge_args as populate_located_bb_exception_edge_args,
    recompute_lowered_block_params, should_include_closure_storage_aliases,
};
use crate::block_py::{
    BbBlock, BbStmt, BlockPyFunction, BlockPyFunctionKind, BlockPyModule, BlockPyStmt, BlockPyTerm,
    CfgBlock, CoreBlockPyExpr, LocatedCoreBlockPyExpr, LocatedName, ModuleNameGen,
};
use crate::passes::{
    BbBlockPyPass, CoreBlockPyPass, CoreBlockPyPassWithYield, LocatedCoreBlockPyPass,
};
use std::collections::HashMap;

pub use exception_pass::lower_try_jump_exception_flow;
pub use strings::normalize_bb_module_strings;

pub(crate) fn lower_yield_in_lowered_core_blockpy_module_bundle(
    module: BlockPyModule<CoreBlockPyPassWithYield>,
) -> BlockPyModule<CoreBlockPyPass> {
    let module =
        module.map_callable_defs(make_eval_order_explicit_in_core_callable_def_without_await);
    let next_hidden_function_id = module
        .callable_defs
        .iter()
        .map(|callable| callable.function_id.0)
        .max()
        .map(|value| value + 1)
        .unwrap_or(0);
    let mut module_name_gen = ModuleNameGen::new(next_hidden_function_id);
    let mut callable_defs = Vec::new();
    for callable in module.callable_defs {
        match callable.kind {
            BlockPyFunctionKind::Function => {
                let qualname = callable.names.qualname.clone();
                callable_defs.push(callable.try_into().unwrap_or_else(|_| {
                    panic!(
                        "core BlockPy yield lowering is not explicit yet: yield-family expr reached the core no-yield boundary for {}",
                        qualname
                    )
                }));
            }
            BlockPyFunctionKind::Generator
            | BlockPyFunctionKind::Coroutine
            | BlockPyFunctionKind::AsyncGenerator => {
                callable_defs.extend(lower_generator_like_function(
                    callable,
                    &mut module_name_gen,
                ));
            }
        }
    }
    BlockPyModule { callable_defs }
}

pub(crate) fn lower_core_blockpy_module_bundle_to_bb_module(
    module: BlockPyModule<LocatedCoreBlockPyPass>,
) -> BlockPyModule<BbBlockPyPass> {
    module.map_callable_defs(lower_core_blockpy_function_to_bb_function)
}

pub(crate) fn lower_core_blockpy_function_to_bb_function(
    lowered: BlockPyFunction<LocatedCoreBlockPyPass>,
) -> BlockPyFunction<BbBlockPyPass> {
    let block_params =
        recompute_lowered_block_params(&lowered, should_include_closure_storage_aliases(&lowered));
    let BlockPyFunction {
        function_id,
        name_gen,
        names,
        kind,
        params,
        blocks,
        doc,
        closure_layout,
        semantic,
    } = lowered;
    BlockPyFunction {
        function_id,
        name_gen,
        names,
        kind,
        params,
        blocks: lower_blockpy_blocks_to_bb_blocks(&blocks, &block_params),
        doc,
        closure_layout,
        semantic,
    }
}

fn lower_blockpy_blocks_to_bb_blocks(
    blocks: &[crate::block_py::CfgBlock<
        BlockPyStmt<CoreBlockPyExpr<LocatedName>, LocatedName>,
        BlockPyTerm<LocatedCoreBlockPyExpr>,
    >],
    block_params: &HashMap<String, Vec<String>>,
) -> Vec<BbBlock> {
    lower_structured_located_blocks_to_bb_blocks(blocks, block_params)
}

pub(super) fn populate_exception_edge_args(
    blocks: &mut [CfgBlock<BbStmt, BlockPyTerm<LocatedCoreBlockPyExpr>>],
) {
    populate_located_bb_exception_edge_args(blocks);
}

#[cfg(test)]
mod test;
