use crate::block_py::{
    BlockPyModule, CodegenBlockPyExpr, CounterDef, CounterId, CounterPoint, IncrementCounter, Meta,
    WithMeta,
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
