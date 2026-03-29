use dp_transform::block_py::{
    BlockArg, BlockPyFunction, BlockPyFunctionKind, BlockPyModule, CodegenBlock, ParamKind,
    ParamSpec,
};
use dp_transform::passes::CodegenBlockPyPass;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Debug)]
pub struct JitFunctionInfo {
    pub entry_params: ParamSpec,
    pub entry_param_names: Vec<String>,
    pub entry_param_default_sources: Vec<Option<ClifEntryParamDefaultSource>>,
    pub ambient_param_names: Vec<String>,
    pub owned_cell_slot_names: Vec<String>,
    pub slot_names: Vec<String>,
    pub blocks: Vec<JitBlockInfo>,
}

#[derive(Clone, Debug)]
pub enum ClifEntryParamDefaultSource {
    Positional(usize),
    KeywordOnly(String),
}

#[derive(Clone, Debug)]
pub struct JitBlockInfo {
    pub runtime_param_names: Vec<String>,
    pub exc_target: Option<usize>,
    pub exc_dispatch: Option<BlockExcDispatchPlan>,
}

#[derive(Clone, Debug)]
pub struct BlockExcDispatchPlan {
    pub target_index: usize,
    pub slot_writes: Vec<(String, BlockExcArgSource)>,
}

#[derive(Clone, Debug)]
pub enum BlockExcArgSource {
    Name(String),
    Exception,
    NoneValue,
}

#[derive(Clone)]
struct RegisteredJitFunction {
    function: BlockPyFunction<CodegenBlockPyPass>,
    info: JitFunctionInfo,
}

type FunctionRegistry = HashMap<PlanKey, RegisteredJitFunction>;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct PlanKey {
    pub module: String,
    pub function_id: usize,
}

static BB_FUNCTION_REGISTRY: OnceLock<Mutex<FunctionRegistry>> = OnceLock::new();

struct ValidatedPreparedBbFunction<'a> {
    function: &'a BlockPyFunction<CodegenBlockPyPass>,
    ambient_param_names: Vec<String>,
    ambient_param_name_set: HashSet<String>,
}

impl<'a> ValidatedPreparedBbFunction<'a> {
    fn new(function: &'a BlockPyFunction<CodegenBlockPyPass>) -> Self {
        let ambient_param_names = function
            .closure_layout()
            .as_ref()
            .map(|layout| layout.ambient_storage_names())
            .unwrap_or_default();
        let ambient_param_name_set = ambient_param_names.iter().cloned().collect();
        Self {
            function,
            ambient_param_names,
            ambient_param_name_set,
        }
    }

    fn jit_param_names_for_block(&self, block: &CodegenBlock) -> Vec<String> {
        block
            .exception_param()
            .into_iter()
            .map(ToString::to_string)
            .collect()
    }

    fn exc_dispatch_plan(&self, block: &CodegenBlock) -> Option<BlockExcDispatchPlan> {
        let exc_edge = block.exc_edge.as_ref()?;
        let target_index = exc_edge.target.index();
        let target_block = &self.function.blocks[target_index];
        let runtime_param_name_set = self
            .jit_param_names_for_block(target_block)
            .into_iter()
            .collect::<HashSet<_>>();
        let full_target_param_names = target_block.param_name_vec();
        let mut slot_writes = Vec::new();
        for (target_param_name, source) in full_target_param_names.iter().zip(exc_edge.args.iter())
        {
            if runtime_param_name_set.contains(target_param_name)
                || self.ambient_param_name_set.contains(target_param_name)
            {
                continue;
            }
            let source = match source {
                BlockArg::Name(name) => BlockExcArgSource::Name(name.clone()),
                BlockArg::CurrentException => BlockExcArgSource::Exception,
                BlockArg::None => BlockExcArgSource::NoneValue,
                BlockArg::AbruptKind(kind) => {
                    panic!(
                        "exception dispatch from {}:{} uses abrupt-kind edge arg {:?} for target param {}",
                        self.function.names.qualname, block.label, kind, target_param_name
                    );
                }
            };
            slot_writes.push((target_param_name.clone(), source));
        }
        Some(BlockExcDispatchPlan {
            target_index,
            slot_writes,
        })
    }

    fn slot_names(&self) -> Vec<String> {
        let mut slot_names = Vec::new();
        let mut seen = HashSet::new();

        for name in &self.ambient_param_names {
            if seen.insert(name.clone()) {
                slot_names.push(name.clone());
            }
        }

        for name in self.function.params.names() {
            if seen.insert(name.clone()) {
                slot_names.push(name);
            }
        }

        for name in self.function.local_cell_slots() {
            if seen.insert(name.clone()) {
                slot_names.push(name);
            }
        }

        for block in &self.function.blocks {
            for name in block.param_names() {
                if seen.insert(name.to_string()) {
                    slot_names.push(name.to_string());
                }
            }
        }

        slot_names
    }

    fn entry_param_default_sources(&self) -> Vec<Option<ClifEntryParamDefaultSource>> {
        let mut next_positional_default = 0usize;
        self.function
            .params
            .params
            .iter()
            .map(|param| {
                if !param.has_default {
                    return None;
                }
                match param.kind {
                    ParamKind::PosOnly | ParamKind::Any => {
                        let index = next_positional_default;
                        next_positional_default += 1;
                        Some(ClifEntryParamDefaultSource::Positional(index))
                    }
                    ParamKind::KwOnly => {
                        Some(ClifEntryParamDefaultSource::KeywordOnly(param.name.clone()))
                    }
                    ParamKind::VarArg | ParamKind::KwArg => None,
                }
            })
            .collect()
    }
}

fn bb_function_registry() -> &'static Mutex<FunctionRegistry> {
    BB_FUNCTION_REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn build_jit_function_info(function: &BlockPyFunction<CodegenBlockPyPass>) -> JitFunctionInfo {
    let function = ValidatedPreparedBbFunction::new(function);
    let slot_names = function.slot_names();
    let owned_cell_slot_names = match function.function.kind {
        BlockPyFunctionKind::Function => {
            let mut names = function
                .function
                .semantic
                .owned_cell_storage_names()
                .into_iter()
                .collect::<Vec<_>>();
            names.sort();
            names
        }
        BlockPyFunctionKind::Generator
        | BlockPyFunctionKind::Coroutine
        | BlockPyFunctionKind::AsyncGenerator => function.function.local_cell_slots(),
    };
    let blocks = function
        .function
        .blocks
        .iter()
        .map(|block| {
            let exc_dispatch = function.exc_dispatch_plan(block);
            let exc_target = exc_dispatch.as_ref().map(|plan| plan.target_index);
            let runtime_param_names = function.jit_param_names_for_block(block);
            JitBlockInfo {
                runtime_param_names,
                exc_target,
                exc_dispatch,
            }
        })
        .collect();
    JitFunctionInfo {
        entry_params: function.function.params.clone(),
        entry_param_names: function.function.params.names(),
        entry_param_default_sources: function.entry_param_default_sources(),
        ambient_param_names: function.ambient_param_names.clone(),
        owned_cell_slot_names,
        slot_names,
        blocks,
    }
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
        let info = build_jit_function_info(function);
        functions.insert(
            key,
            RegisteredJitFunction {
                function: function.clone(),
                info,
            },
        );
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
        .map(|registered| registered.function.clone())
}

pub fn lookup_registered_jit_function(
    module_name: &str,
    function_id: usize,
) -> Option<(BlockPyFunction<CodegenBlockPyPass>, JitFunctionInfo)> {
    let registry = bb_function_registry().lock().ok()?;
    registry
        .get(&PlanKey {
            module: module_name.to_string(),
            function_id,
        })
        .map(|registered| (registered.function.clone(), registered.info.clone()))
}
