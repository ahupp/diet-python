use crate::block_py::{
    compute_storage_layout_from_semantics, BlockArg, BlockPyFunction, BlockPyLabel,
    BlockPyLinearPass, BlockPyModule, BlockPyPass, BlockPySemanticExprNode, BlockPyTerm, PassBlock,
    PassExpr,
};

pub(crate) fn validate_module<P: BlockPyPass>(module: &BlockPyModule<P>) -> Result<(), String>
where
    P: BlockPyLinearPass,
    PassExpr<P>: BlockPySemanticExprNode,
{
    for function in &module.callable_defs {
        validate_function(function)?;
    }
    Ok(())
}

fn validate_function<P: BlockPyPass>(function: &BlockPyFunction<P>) -> Result<(), String>
where
    P: BlockPyLinearPass,
    PassExpr<P>: BlockPySemanticExprNode,
{
    let qualname = function.names.qualname.as_str();
    validate_storage_layout_scoping(function, qualname)?;
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
            for (target_param_name, source) in target_block
                .param_name_vec()
                .iter()
                .zip(exc_edge.args.iter())
            {
                if let BlockArg::AbruptKind(kind) = source {
                    return Err(format!(
                        "exception dispatch from {}:{} uses abrupt-kind edge arg {:?} for target param {}",
                        qualname, block.label, kind, target_param_name
                    ));
                }
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

fn validate_storage_layout_scoping<P: BlockPyPass>(
    function: &BlockPyFunction<P>,
    qualname: &str,
) -> Result<(), String>
where
    P: BlockPyLinearPass,
    PassExpr<P>: BlockPySemanticExprNode,
{
    let expected_layout = compute_storage_layout_from_semantics(function);

    let Some(layout) = function.storage_layout.as_ref() else {
        if expected_layout.is_none() {
            return Ok(());
        }
        return Err(format!(
            "closure layout missing for {} despite semantic closure state",
            qualname
        ));
    };

    let Some(expected_layout) = expected_layout else {
        return Ok(());
    };

    for expected_slot in &expected_layout.cellvars {
        let Some(actual_slot) = layout
            .cellvars
            .iter()
            .find(|slot| slot.logical_name == expected_slot.logical_name)
        else {
            return Err(format!(
                "closure layout for {} is missing owner cell {}; actual cellvars: {:?}",
                qualname,
                expected_slot.logical_name,
                layout
                    .cellvars
                    .iter()
                    .map(|slot| format!("{}->{}", slot.logical_name, slot.storage_name))
                    .collect::<Vec<_>>()
            ));
        };
        if actual_slot.storage_name != expected_slot.storage_name {
            return Err(format!(
                "closure layout for {} has owner cell {} stored as {}, but semantic info expects {}; actual cellvars: {:?}",
                qualname,
                expected_slot.logical_name,
                actual_slot.storage_name,
                expected_slot.storage_name,
                layout
                    .cellvars
                    .iter()
                    .map(|slot| format!("{}->{}", slot.logical_name, slot.storage_name))
                    .collect::<Vec<_>>()
            ));
        }
    }

    for expected_slot in &expected_layout.freevars {
        if !layout
            .freevars
            .iter()
            .any(|slot| slot.logical_name == expected_slot.logical_name)
        {
            return Err(format!(
                "closure layout for {} is missing freevar {}; actual freevars: {:?}",
                qualname,
                expected_slot.logical_name,
                layout
                    .freevars
                    .iter()
                    .map(|slot| format!("{}->{}", slot.logical_name, slot.storage_name))
                    .collect::<Vec<_>>()
            ));
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
