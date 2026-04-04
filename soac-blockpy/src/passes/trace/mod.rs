use crate::block_py::{
    core_call_expr_with_meta, literal_expr, BlockPyFunction, BlockPyModule, CallArgPositional,
    CodegenBlockPyExpr, CoreStringLiteral, CounterDef, CounterId, CounterScope, CounterSite,
    IncrementCounter, Load, LocatedCoreBlockPyExpr, LocatedName, Meta, NameLocation, WithMeta,
};
use crate::passes::CodegenBlockPyPass;
use std::collections::HashMap;
use std::env;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TraceConfig {
    pub(crate) qualname_filter: Option<String>,
    pub(crate) include_params: bool,
}

pub(crate) fn parse_trace_env() -> Option<TraceConfig> {
    let raw = env::var("DIET_PYTHON_BB_TRACE").ok()?;
    parse_trace_config(raw.as_str())
}

pub(crate) fn global_load_counter_instrumentation_enabled() -> bool {
    env::var("DIET_PYTHON_GLOBAL_LOAD_COUNTERS")
        .map(|raw| {
            let trimmed = raw.trim();
            !(trimmed.is_empty() || trimmed == "0")
        })
        .unwrap_or(false)
}

pub(crate) fn parse_trace_config(raw: &str) -> Option<TraceConfig> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "0" {
        return None;
    }
    let (selector, include_params) = if let Some(stripped) = trimmed.strip_suffix(":params") {
        (stripped.trim(), true)
    } else {
        (trimmed, false)
    };
    let qualname_filter = match selector {
        "" | "1" | "*" | "all" => None,
        value => Some(value.to_string()),
    };
    Some(TraceConfig {
        qualname_filter,
        include_params,
    })
}

pub(crate) fn instrument_bb_module_for_trace(
    module: &mut BlockPyModule<CodegenBlockPyPass>,
    config: &TraceConfig,
) {
    let global_names = module.global_names.clone();
    let module_constants = &mut module.module_constants;
    for function in &mut module.callable_defs {
        if let Some(filter) = config.qualname_filter.as_ref() {
            if function.names.qualname != *filter {
                continue;
            }
        }
        let qualname = function.names.qualname.clone();
        let locator = PreparedTraceNameLocator::new(function, global_names.as_slice());
        for block in &mut function.blocks {
            let block_params = block.param_name_vec();
            let trace_expr = if config.include_params && !block_params.is_empty() {
                helper_call_expr(
                    "bb_trace_enter",
                    vec![
                        string_literal_expr(module_constants, qualname.as_str()),
                        string_literal_expr(module_constants, block.label.to_string().as_str()),
                        param_pairs_expr(module_constants, &locator, block_params.as_slice()),
                    ],
                )
            } else {
                helper_call_expr(
                    "bb_trace_enter",
                    vec![
                        string_literal_expr(module_constants, qualname.as_str()),
                        string_literal_expr(module_constants, block.label.to_string().as_str()),
                    ],
                )
            };
            block.body.insert(0, trace_expr);
        }
    }
}

pub fn instrument_bb_module_with_block_entry_counters(
    module: &mut BlockPyModule<CodegenBlockPyPass>,
) {
    let mut next_counter_id = module.counter_defs.len();
    for function in &mut module.callable_defs {
        for block in &mut function.blocks {
            let counter_id = CounterId(next_counter_id);
            next_counter_id += 1;
            module.counter_defs.push(CounterDef {
                id: counter_id,
                scope: CounterScope::This,
                kind: "block_entry".to_string(),
                site: CounterSite::BlockEntry {
                    function_id: function.function_id,
                    block_label: block.label,
                },
            });
            block.body.insert(
                0,
                CodegenBlockPyExpr::from(
                    IncrementCounter::new(counter_id).with_meta(Meta::synthetic()),
                ),
            );
        }
    }
}

pub fn instrument_bb_module_with_refcount_counters(
    module: &mut BlockPyModule<CodegenBlockPyPass>,
    scope: CounterScope,
) -> Result<(), String> {
    match scope {
        CounterScope::This => {
            return Err(
                "refcount counters do not yet support CounterScope::This; use Function or Global"
                    .to_string(),
            );
        }
        CounterScope::Function => {
            let mut next_counter_id = module.counter_defs.len();
            let function_ids = module
                .callable_defs
                .iter()
                .map(|function| function.function_id)
                .collect::<Vec<_>>();
            for function_id in function_ids {
                for kind in ["runtime_incref", "runtime_decref"] {
                    let site = CounterSite::Runtime {
                        function_id: Some(function_id),
                    };
                    if module.counter_defs.iter().any(|counter| {
                        counter.scope == scope && counter.kind == kind && counter.site == site
                    }) {
                        continue;
                    }
                    let counter_id = CounterId(next_counter_id);
                    next_counter_id += 1;
                    module.counter_defs.push(CounterDef {
                        id: counter_id,
                        scope,
                        kind: kind.to_string(),
                        site,
                    });
                }
            }
        }
        CounterScope::Global => {
            let mut next_counter_id = module.counter_defs.len();
            for kind in ["runtime_incref", "runtime_decref"] {
                let site = CounterSite::Runtime { function_id: None };
                if module.counter_defs.iter().any(|counter| {
                    counter.scope == scope && counter.kind == kind && counter.site == site
                }) {
                    continue;
                }
                let counter_id = CounterId(next_counter_id);
                next_counter_id += 1;
                module.counter_defs.push(CounterDef {
                    id: counter_id,
                    scope,
                    kind: kind.to_string(),
                    site,
                });
            }
        }
    }
    Ok(())
}

pub fn instrument_bb_module_with_global_load_counters(
    module: &mut BlockPyModule<CodegenBlockPyPass>,
) {
    let mut next_counter_id = module.counter_defs.len();
    for kind in ["global_load_hit", "global_load_miss"] {
        let site = CounterSite::Runtime { function_id: None };
        if module.counter_defs.iter().any(|counter| {
            counter.scope == CounterScope::Global && counter.kind == kind && counter.site == site
        }) {
            continue;
        }
        let counter_id = CounterId(next_counter_id);
        next_counter_id += 1;
        module.counter_defs.push(CounterDef {
            id: counter_id,
            scope: CounterScope::Global,
            kind: kind.to_string(),
            site,
        });
    }
}

struct PreparedTraceNameLocator {
    local_slots: HashMap<String, u32>,
    existing_locations: HashMap<String, NameLocation>,
    captured_cell_slots: HashMap<String, u32>,
    owned_cell_slots: HashMap<String, u32>,
    global_slots: HashMap<String, u32>,
}

impl PreparedTraceNameLocator {
    fn new(function: &BlockPyFunction<CodegenBlockPyPass>, global_names: &[String]) -> Self {
        let mut local_slots = function
            .storage_layout
            .as_ref()
            .map(|layout| {
                layout
                    .stack_slots()
                    .iter()
                    .enumerate()
                    .map(|(slot, name)| (name.clone(), slot as u32))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();
        for (slot, name) in function.params.names().into_iter().enumerate() {
            local_slots.entry(name).or_insert(slot as u32);
        }
        let mut existing_locations = HashMap::new();
        for block in &function.blocks {
            for stmt in &block.body {
                if let CodegenBlockPyExpr::Store(store) = stmt {
                    existing_locations
                        .entry(store.name.id.to_string())
                        .or_insert(store.name.location);
                }
            }
        }
        let captured_cell_slots = function
            .storage_layout
            .as_ref()
            .map(|layout| {
                let mut slots = HashMap::new();
                for (slot, closure_slot) in layout.freevars.iter().enumerate() {
                    slots.insert(closure_slot.storage_name.clone(), slot as u32);
                    slots.insert(closure_slot.logical_name.clone(), slot as u32);
                }
                slots
            })
            .unwrap_or_default();
        let owned_cell_slots = function
            .storage_layout
            .as_ref()
            .map(|layout| {
                let mut slots = HashMap::new();
                for (slot, closure_slot) in layout
                    .cellvars
                    .iter()
                    .chain(layout.runtime_cells.iter())
                    .enumerate()
                {
                    slots.insert(closure_slot.storage_name.clone(), slot as u32);
                    slots.insert(closure_slot.logical_name.clone(), slot as u32);
                }
                slots
            })
            .unwrap_or_default();
        let global_slots = global_names
            .iter()
            .enumerate()
            .map(|(slot, name)| (name.clone(), slot as u32))
            .collect::<HashMap<_, _>>();
        Self {
            local_slots,
            existing_locations,
            captured_cell_slots,
            owned_cell_slots,
            global_slots,
        }
    }

    fn load_name(&self, id: &str) -> LocatedName {
        let location = if let Some(slot) = self.local_slots.get(id).copied() {
            NameLocation::local(slot)
        } else if let Some(location) = self.existing_locations.get(id).copied() {
            location
        } else if let Some(slot) = self.captured_cell_slots.get(id).copied() {
            NameLocation::closure_cell(slot)
        } else if let Some(slot) = self.owned_cell_slots.get(id).copied() {
            NameLocation::owned_cell(slot)
        } else {
            let slot = self
                .global_slots
                .get(id)
                .copied()
                .unwrap_or_else(|| panic!("trace locator missing global slot for {id}"));
            NameLocation::global(slot)
        };
        LocatedName {
            id: id.into(),
            location,
        }
    }
}

fn helper_call_expr(helper_name: &str, args: Vec<CodegenBlockPyExpr>) -> CodegenBlockPyExpr {
    let meta = Meta::synthetic();
    let func = Load::new(LocatedName {
        id: helper_name.into(),
        location: NameLocation::RuntimeName,
    })
    .with_meta(meta.clone())
    .into();
    core_call_expr_with_meta(
        func,
        meta.node_index,
        meta.range,
        args.into_iter()
            .map(CallArgPositional::Positional)
            .collect(),
        Vec::new(),
    )
}

fn string_literal_expr(
    module_constants: &mut Vec<LocatedCoreBlockPyExpr>,
    value: &str,
) -> CodegenBlockPyExpr {
    let meta = Meta::synthetic();
    let index = u32::try_from(module_constants.len())
        .expect("trace module constant count should fit in u32");
    module_constants.push(literal_expr(
        CoreStringLiteral {
            value: value.to_string(),
        },
        meta.clone(),
    ));
    crate::block_py::Load::new(crate::block_py::LocatedName {
        id: format!("__dp_constant_{index}").into(),
        location: NameLocation::Constant(index),
    })
    .with_meta(meta)
    .into()
}

fn tuple_expr(values: Vec<CodegenBlockPyExpr>) -> CodegenBlockPyExpr {
    helper_call_expr("tuple_values", values)
}

fn param_pairs_expr(
    module_constants: &mut Vec<LocatedCoreBlockPyExpr>,
    locator: &PreparedTraceNameLocator,
    params: &[String],
) -> CodegenBlockPyExpr {
    tuple_expr(
        params
            .iter()
            .map(|param| {
                let name = locator.load_name(param);
                let meta = Meta::synthetic();
                tuple_expr(vec![
                    string_literal_expr(module_constants, param),
                    Load::new(name).with_meta(meta).into(),
                ])
            })
            .collect(),
    )
}

#[cfg(test)]
mod test;
