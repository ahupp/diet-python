use crate::block_py::{
    BlockPyFunction, BlockPyLabel, BlockPyModule, BlockPyPass, BlockPyTerm, PassBlock,
};

pub fn validate_module<P: BlockPyPass>(module: &BlockPyModule<P>) -> Result<(), String> {
    for function in &module.callable_defs {
        validate_function(function)?;
    }
    Ok(())
}

fn validate_function<P: BlockPyPass>(function: &BlockPyFunction<P>) -> Result<(), String> {
    let qualname = function.names.qualname.as_str();
    for (index, block) in function.blocks.iter().enumerate() {
        let expected_label = BlockPyLabel::from_index(index);
        if block.label != expected_label {
            return Err(format!(
                "non-dense block label {} at {}:{}, expected {}",
                block.label, qualname, index, expected_label
            ));
        }
    }

    for block in &function.blocks {
        if let Some(exc_edge) = block.exc_edge.as_ref() {
            let target_block = lookup_known_block(
                function,
                exc_edge.target,
                qualname,
                block.label,
                "exception target",
            )?;
            if exc_edge.args.len() != target_block.param_name_vec().len() {
                return Err(format!(
                    "exception dispatch from {}:{} has {} explicit edge args for target {} with {} full params",
                    qualname,
                    block.label,
                    exc_edge.args.len(),
                    target_block.label,
                    target_block.param_name_vec().len()
                ));
            }
        }
        match &block.term {
            BlockPyTerm::Jump(target) => {
                lookup_known_block(
                    function,
                    target.target,
                    qualname,
                    block.label,
                    "jump target",
                )?;
            }
            BlockPyTerm::IfTerm(if_term) => {
                lookup_known_block(
                    function,
                    if_term.then_label,
                    qualname,
                    block.label,
                    "then target",
                )?;
                lookup_known_block(
                    function,
                    if_term.else_label,
                    qualname,
                    block.label,
                    "else target",
                )?;
            }
            BlockPyTerm::BranchTable(branch) => {
                for target in &branch.targets {
                    lookup_known_block(
                        function,
                        *target,
                        qualname,
                        block.label,
                        "br_table target",
                    )?;
                }
                lookup_known_block(
                    function,
                    branch.default_label,
                    qualname,
                    block.label,
                    "br_table default target",
                )?;
            }
            BlockPyTerm::Raise(_) | BlockPyTerm::Return(_) => {}
        }
    }
    Ok(())
}

fn lookup_known_block<'a, P: BlockPyPass>(
    function: &'a BlockPyFunction<P>,
    label: BlockPyLabel,
    qualname: &str,
    block_label: BlockPyLabel,
    label_kind: &str,
) -> Result<&'a PassBlock<P>, String> {
    let Some(target_block) = function.blocks.get(label.index()) else {
        return Err(format!(
            "unknown {label_kind} {label} in {}:{}",
            qualname, block_label
        ));
    };
    if target_block.label == label {
        return Ok(target_block);
    }
    Err(format!(
        "unknown {label_kind} {label} in {}:{}",
        qualname, block_label
    ))
}
