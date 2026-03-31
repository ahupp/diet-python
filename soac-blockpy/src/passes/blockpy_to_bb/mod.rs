mod exception_pass;
mod strings;

use super::blockpy_generators::lower_generator_like_function;
use super::core_eval_order::make_eval_order_explicit_in_core_callable_def_without_await;
use crate::block_py::{BlockPyFunctionKind, BlockPyModule, ExprTryMap, ModuleNameGen};
use crate::passes::{CoreBlockPyPass, CoreBlockPyPassWithYield};

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
                callable_defs.push(
                    ExprTryMap::<
                        CoreBlockPyPassWithYield,
                        CoreBlockPyPass,
                        crate::block_py::CoreBlockPyExprWithYield,
                    >::without_yield()
                        .try_map_fn(callable)
                        .unwrap_or_else(|_| {
                            panic!(
                                "core BlockPy yield lowering is not explicit yet: yield-family expr reached the core no-yield boundary for {}",
                                qualname
                            )
                        }),
                );
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

#[cfg(test)]
mod test;
