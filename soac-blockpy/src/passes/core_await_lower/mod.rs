use crate::block_py::{BlockPyModule, BlockPyModuleMap};
use crate::passes::{CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield};

struct CoreAwaitLoweringMap;

impl BlockPyModuleMap<CoreBlockPyPassWithAwaitAndYield, CoreBlockPyPassWithYield>
    for CoreAwaitLoweringMap
{
}

pub(crate) fn lower_awaits_in_core_blockpy_module(
    module: BlockPyModule<CoreBlockPyPassWithAwaitAndYield>,
) -> BlockPyModule<CoreBlockPyPassWithYield> {
    module.map_module(&CoreAwaitLoweringMap)
}

#[cfg(test)]
mod test;
