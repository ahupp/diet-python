use crate::block_py::{
    BlockPyModule, CodegenBlockPyExpr, CounterDef, CounterId, CounterPoint, CounterScope,
    IncrementCounter, Meta, WithMeta,
};
use crate::passes::CodegenBlockPyPass;

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
                point: CounterPoint::BlockEntry {
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
                for point in [
                    CounterPoint::RuntimeIncref {
                        function_id: Some(function_id),
                    },
                    CounterPoint::RuntimeDecref {
                        function_id: Some(function_id),
                    },
                ] {
                    if module
                        .counter_defs
                        .iter()
                        .any(|counter| counter.scope == scope && counter.point == point)
                    {
                        continue;
                    }
                    let counter_id = CounterId(next_counter_id);
                    next_counter_id += 1;
                    module.counter_defs.push(CounterDef {
                        id: counter_id,
                        scope,
                        point,
                    });
                }
            }
        }
        CounterScope::Global => {
            let mut next_counter_id = module.counter_defs.len();
            for point in [
                CounterPoint::RuntimeIncref { function_id: None },
                CounterPoint::RuntimeDecref { function_id: None },
            ] {
                if module
                    .counter_defs
                    .iter()
                    .any(|counter| counter.scope == scope && counter.point == point)
                {
                    continue;
                }
                let counter_id = CounterId(next_counter_id);
                next_counter_id += 1;
                module.counter_defs.push(CounterDef {
                    id: counter_id,
                    scope,
                    point,
                });
            }
        }
    }
    Ok(())
}
