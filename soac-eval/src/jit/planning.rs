use dp_transform::block_py::{BlockArg, BlockPyFunction, BlockPyModule, CodegenBlock};
use dp_transform::passes::CodegenBlockPyPass;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug)]
pub struct JitBlockInfo {
    pub runtime_param_names: Vec<String>,
    pub exc_dispatch: Option<BlockExcDispatchPlan>,
}

#[derive(Clone, Debug)]
pub struct BlockExcDispatchPlan {
    pub target_index: usize,
    pub slot_writes: Vec<(String, BlockArg)>,
}

type FunctionRegistry = HashMap<PlanKey, BlockPyFunction<CodegenBlockPyPass>>;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PlanKey {
    pub module: String,
    pub function_id: usize,
}

static BB_FUNCTION_REGISTRY: OnceLock<Mutex<FunctionRegistry>> = OnceLock::new();

pub fn jit_param_names_for_block(block: &CodegenBlock) -> Vec<String> {
    block
        .exception_param()
        .into_iter()
        .map(ToString::to_string)
        .collect()
}

pub fn exc_dispatch_plan(
    function: &BlockPyFunction<CodegenBlockPyPass>,
    block: &CodegenBlock,
) -> Option<BlockExcDispatchPlan> {
    let exc_edge = block.exc_edge.as_ref()?;
    let target_index = exc_edge.target.index();
    let target_block = &function.blocks[target_index];
    let ambient_param_name_set = function
        .storage_layout()
        .as_ref()
        .map(|layout| {
            layout
                .ambient_storage_names()
                .into_iter()
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    let runtime_param_name_set = jit_param_names_for_block(target_block)
        .into_iter()
        .collect::<HashSet<_>>();
    let full_target_param_names = target_block.param_name_vec();
    let mut slot_writes = Vec::new();
    for (target_param_name, source) in full_target_param_names.iter().zip(exc_edge.args.iter()) {
        if runtime_param_name_set.contains(target_param_name)
            || ambient_param_name_set.contains(target_param_name)
        {
            continue;
        }
        slot_writes.push((target_param_name.clone(), source.clone()));
    }
    Some(BlockExcDispatchPlan {
        target_index,
        slot_writes,
    })
}

pub fn jit_block_info(
    function: &BlockPyFunction<CodegenBlockPyPass>,
    block: &CodegenBlock,
) -> JitBlockInfo {
    let exc_dispatch = exc_dispatch_plan(function, block);
    let runtime_param_names = jit_param_names_for_block(block);
    JitBlockInfo {
        runtime_param_names,
        exc_dispatch,
    }
}

fn bb_function_registry() -> &'static Mutex<FunctionRegistry> {
    BB_FUNCTION_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn register_clif_module_plans(
    module_name: &str,
    module: &BlockPyModule<CodegenBlockPyPass>,
) -> Result<(), String> {
    let mut functions = HashMap::new();
    for function in &module.callable_defs {
        let key = PlanKey {
            module: module_name.to_string(),
            function_id: function.function_id.0,
        };
        functions.insert(key, function.clone());
    }

    let mut function_registry = bb_function_registry()
        .lock()
        .map_err(|_| "failed to lock bb function registry".to_string())?;
    function_registry.retain(|key, _| key.module != module_name);
    function_registry.extend(functions);
    Ok(())
}

pub fn lookup_blockpy_function(
    module_name: &str,
    function_id: usize,
) -> Option<BlockPyFunction<CodegenBlockPyPass>> {
    let registry = bb_function_registry().lock().ok()?;
    registry
        .get(&PlanKey {
            module: module_name.to_string(),
            function_id,
        })
        .cloned()
}
