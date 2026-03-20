use crate::basic_block::bb_ir::{BbBlock, BbBlockMeta, BbStmt};
use crate::basic_block::block_py::{
    BbBlockPyPass, BlockPyFunction, BlockPyLabel, BlockPyModule, BlockPyStmt, BlockPyTerm,
};
use std::collections::HashSet;

pub fn lower_try_jump_exception_flow(
    module: &BlockPyModule<BbBlockPyPass>,
) -> Result<BlockPyModule<BbBlockPyPass>, String> {
    let mut lowered = module.clone();
    for function in &mut lowered.callable_defs {
        lower_function_try_jump_exception_flow(function)?;
    }
    Ok(lowered)
}

fn lower_function_try_jump_exception_flow(
    function: &mut BlockPyFunction<BbBlockPyPass>,
) -> Result<(), String> {
    let label_set: HashSet<&str> = function
        .blocks
        .iter()
        .map(|block| block.label.as_str())
        .collect();
    validate_function_labels(function, &label_set)?;

    // Canonicalize exception-edge blocks so each potentially-raising expression
    // step sits in its own block. This keeps per-expression exception checks
    // explicit in CFG shape (op-block -> jump -> ... -> term-block), which
    // allows the JIT fast path to dispatch exceptions directly from each step.
    split_exception_blocks_for_expr_checks(function);

    Ok(())
}

fn split_exception_blocks_for_expr_checks(function: &mut BlockPyFunction<BbBlockPyPass>) {
    let mut used_labels: HashSet<BlockPyLabel> = function
        .blocks
        .iter()
        .map(|block| block.label.clone())
        .collect();
    let mut fresh_index: usize = 0;
    let mut out = Vec::with_capacity(function.blocks.len());

    for block in std::mem::take(&mut function.blocks) {
        if block.meta.exc_target_label.is_none() || block.body.is_empty() {
            out.push(block);
            continue;
        }

        let mut known_names = block.meta.params.clone();
        let mut current_label = block.label.clone();
        let edge_target = block.meta.exc_target_label.clone();
        let edge_exc_name = block.meta.exc_name.clone();
        let mut ops = block.body.into_iter().peekable();
        let mut segment_start_names = known_names.clone();

        let mut segment_ops: Vec<BbStmt> = Vec::new();
        while let Some(op) = ops.next() {
            let ends_segment = op_updates_exception_state(&op) && ops.peek().is_some();
            segment_ops.push(op.clone());
            apply_op_effect_to_known_names(&op, &mut known_names);

            if ends_segment {
                let next_label =
                    unique_exc_split_label(&mut used_labels, current_label.as_str(), fresh_index);
                fresh_index += 1;
                out.push(BbBlock {
                    label: current_label.clone(),
                    body: std::mem::take(&mut segment_ops),
                    term: BlockPyTerm::Jump(next_label.clone()),
                    meta: BbBlockMeta {
                        params: segment_start_names.clone(),
                        exc_target_label: edge_target.clone(),
                        exc_name: edge_exc_name.clone(),
                    },
                });
                current_label = next_label;
                segment_start_names = known_names.clone();
            }

            if ops.peek().is_none() {
                out.push(BbBlock {
                    label: current_label.clone(),
                    body: std::mem::take(&mut segment_ops),
                    term: block.term.clone(),
                    meta: BbBlockMeta {
                        params: segment_start_names.clone(),
                        exc_target_label: edge_target.clone(),
                        exc_name: edge_exc_name.clone(),
                    },
                });
            }
        }
    }

    function.blocks = out;
}

fn op_updates_exception_state(op: &BbStmt) -> bool {
    matches!(op, BlockPyStmt::Assign(_) | BlockPyStmt::Delete(_))
}

fn unique_exc_split_label(
    used_labels: &mut HashSet<BlockPyLabel>,
    base_label: &str,
    index_seed: usize,
) -> BlockPyLabel {
    let mut index = index_seed;
    loop {
        let candidate = BlockPyLabel::from(format!("{base_label}__excchk_{index}"));
        if used_labels.insert(candidate.clone()) {
            return candidate;
        }
        index += 1;
    }
}

fn apply_op_effect_to_known_names(op: &BbStmt, known_names: &mut Vec<String>) {
    match op {
        BlockPyStmt::Assign(assign) => {
            let target = assign.target.id.to_string();
            if !known_names.iter().any(|name| name == &target) {
                known_names.push(target);
            }
        }
        BlockPyStmt::Expr(_) => {}
        BlockPyStmt::Delete(delete) => {
            let target_name = delete.target.id.to_string();
            known_names.retain(|existing| existing != &target_name);
        }
        BlockPyStmt::If(_) => {
            panic!("structured BlockPy If is not allowed in BbBlock.body")
        }
    }
}

fn validate_function_labels(
    function: &BlockPyFunction<BbBlockPyPass>,
    labels: &HashSet<&str>,
) -> Result<(), String> {
    let entry_label = function.entry_label();
    if !labels.contains(entry_label) {
        return Err(format!(
            "missing entry label {} in {}",
            entry_label, function.names.qualname
        ));
    }
    for block in &function.blocks {
        if let Some(exc_target_label) = block.meta.exc_target_label.as_ref() {
            if !labels.contains(exc_target_label.as_str()) {
                return Err(format!(
                    "unknown exception target {exc_target_label} in {}:{}",
                    function.names.qualname, block.label
                ));
            }
        }
        match &block.term {
            BlockPyTerm::Jump(target) => {
                ensure_known_label(labels, target, function, &block.label, "jump target")?
            }
            BlockPyTerm::IfTerm(if_term) => {
                let then_label = &if_term.then_label;
                let else_label = &if_term.else_label;
                ensure_known_label(labels, then_label, function, &block.label, "then target")?;
                ensure_known_label(labels, else_label, function, &block.label, "else target")?;
            }
            BlockPyTerm::BranchTable(branch) => {
                for target in &branch.targets {
                    ensure_known_label(labels, target, function, &block.label, "br_table target")?;
                }
                ensure_known_label(
                    labels,
                    &branch.default_label,
                    function,
                    &block.label,
                    "br_table default target",
                )?;
            }
            BlockPyTerm::Raise(_) | BlockPyTerm::Return(_) => {}
            BlockPyTerm::TryJump(_) => {
                return Err(format!(
                    "unexpected TryJump in BB function {}:{}",
                    function.names.qualname, block.label
                ));
            }
        }
    }
    Ok(())
}

fn ensure_known_label(
    labels: &HashSet<&str>,
    label: &str,
    function: &BlockPyFunction<BbBlockPyPass>,
    block_label: &str,
    label_kind: &str,
) -> Result<(), String> {
    if labels.contains(label) {
        return Ok(());
    }
    Err(format!(
        "unknown {label_kind} {label} in {}:{}",
        function.names.qualname, block_label
    ))
}

#[cfg(test)]
mod tests {
    use super::lower_try_jump_exception_flow;
    use crate::basic_block::bb_ir::{BbBlock, BbBlockMeta};
    use crate::basic_block::block_py::{
        BlockPyLabel, BlockPyTerm, CoreBlockPyExprWithoutAwaitOrYield,
    };
    use crate::{transform_str_to_bb_ir_with_options, Options};

    #[test]
    fn preserves_existing_exception_edges() {
        let source = r#"
def f(x):
    return x
"#;
        let mut module = transform_str_to_bb_ir_with_options(source, Options::for_test())
            .expect("lowering must succeed")
            .expect("bb module must exist");
        let (body_label, except_label) = {
            let function = module
                .callable_defs
                .iter_mut()
                .find(|function| function.names.qualname == "f")
                .expect("must contain f");
            let body_label = BlockPyLabel::from("_dp_manual_body");
            let except_label = BlockPyLabel::from("_dp_manual_except");

            function.blocks.push(BbBlock {
                label: body_label.clone(),
                body: vec![],
                term: BlockPyTerm::<CoreBlockPyExprWithoutAwaitOrYield>::Return(None),
                meta: BbBlockMeta {
                    params: vec![],
                    exc_target_label: Some(except_label.clone()),
                    exc_name: Some("_dp_try_exc_manual".to_string()),
                },
            });
            function.blocks.push(BbBlock {
                label: except_label.clone(),
                body: vec![],
                term: BlockPyTerm::<CoreBlockPyExprWithoutAwaitOrYield>::Return(None),
                meta: BbBlockMeta::default(),
            });
            (body_label, except_label)
        };

        let lowered = lower_try_jump_exception_flow(&module).expect("pass should succeed");
        let lowered_function = lowered
            .callable_defs
            .iter()
            .find(|candidate| candidate.names.qualname == "f")
            .expect("must contain lowered f");
        let body_block = lowered_function
            .blocks
            .iter()
            .find(|block| block.label == body_label)
            .expect("body block must exist");
        assert_eq!(
            body_block
                .meta
                .exc_target_label
                .as_ref()
                .map(BlockPyLabel::as_str),
            Some(except_label.as_str()),
            "body region should dispatch to except block on exception"
        );
        assert_eq!(
            body_block.meta.exc_name.as_deref(),
            Some("_dp_try_exc_manual"),
            "exception binding name should be attached to body region"
        );
    }

    #[test]
    fn rejects_try_jump_with_unknown_label() {
        let source = r#"
def f():
    return 1
"#;
        let mut module = transform_str_to_bb_ir_with_options(source, Options::for_test())
            .expect("lowering must succeed")
            .expect("bb module must exist");
        let function = module
            .callable_defs
            .first_mut()
            .expect("must contain function");
        function.blocks[0].meta.exc_target_label = Some(BlockPyLabel::from("missing_except"));

        let err = lower_try_jump_exception_flow(&module).expect_err("must reject unknown labels");
        assert!(
            err.contains("unknown exception target"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn splits_exception_edge_block_into_one_op_segments() {
        let source = r#"
def f():
    a = 1
    b = 2
    return b
"#;
        let mut module = transform_str_to_bb_ir_with_options(source, Options::for_test())
            .expect("lowering must succeed")
            .expect("bb module must exist");
        let function = module
            .callable_defs
            .iter_mut()
            .find(|function| function.names.qualname == "f")
            .expect("must contain f");
        let block_index = function
            .blocks
            .iter()
            .position(|block| block.body.len() >= 2)
            .expect("must contain multi-op block");
        let original_label = function.blocks[block_index].label.clone();
        let except_label = BlockPyLabel::from("_dp_manual_except_split");
        function.blocks.push(BbBlock {
            label: except_label.clone(),
            body: vec![],
            term: BlockPyTerm::<CoreBlockPyExprWithoutAwaitOrYield>::Return(None),
            meta: BbBlockMeta::default(),
        });
        function.blocks[block_index].meta.exc_target_label = Some(except_label.clone());
        function.blocks[block_index].meta.exc_name = Some("_dp_try_exc_split".to_string());

        let lowered = lower_try_jump_exception_flow(&module).expect("pass should succeed");
        let lowered_function = lowered
            .callable_defs
            .iter()
            .find(|candidate| candidate.names.qualname == "f")
            .expect("must contain lowered f");

        let first = lowered_function
            .blocks
            .iter()
            .find(|block| block.label == original_label)
            .expect("split must keep original block label");
        assert_eq!(first.body.len(), 1, "first split block must contain one op");
        assert!(
            matches!(first.term, BlockPyTerm::Jump(_)),
            "split op block must jump to next split block"
        );
        assert_eq!(
            first
                .meta
                .exc_target_label
                .as_ref()
                .map(BlockPyLabel::as_str),
            Some(except_label.as_str()),
            "split block must preserve exception edge target"
        );

        let split_tail = lowered_function
            .blocks
            .iter()
            .find(|block| block.label.contains("__excchk_"))
            .expect("must contain split tail block");
        assert!(
            split_tail.body.len() <= 1,
            "split tail block should not aggregate ops"
        );
    }

    #[test]
    fn keeps_pure_expr_ops_grouped_until_local_state_changes() {
        let source = r#"
def f():
    x()
    y()
    z = 1
    w()
"#;
        let mut module = transform_str_to_bb_ir_with_options(source, Options::for_test())
            .expect("lowering must succeed")
            .expect("bb module must exist");
        let function = module
            .callable_defs
            .iter_mut()
            .find(|function| function.names.qualname == "f")
            .expect("must contain f");
        let block_index = function
            .blocks
            .iter()
            .position(|block| block.body.len() >= 4)
            .expect("must contain multi-op block");
        let original_label = function.blocks[block_index].label.clone();
        let except_label = BlockPyLabel::from("_dp_manual_except_group");
        function.blocks.push(BbBlock {
            label: except_label.clone(),
            body: vec![],
            term: BlockPyTerm::<CoreBlockPyExprWithoutAwaitOrYield>::Return(None),
            meta: BbBlockMeta::default(),
        });
        function.blocks[block_index].meta.exc_target_label = Some(except_label.clone());
        function.blocks[block_index].meta.exc_name = Some("_dp_try_exc_group".to_string());

        let lowered = lower_try_jump_exception_flow(&module).expect("pass should succeed");
        let lowered_function = lowered
            .callable_defs
            .iter()
            .find(|candidate| candidate.names.qualname == "f")
            .expect("must contain lowered f");

        let first = lowered_function
            .blocks
            .iter()
            .find(|block| block.label == original_label)
            .expect("lowered entry block must exist");
        assert_eq!(
            first.body.len(),
            3,
            "pure expr ops should remain grouped until the local assignment"
        );
        assert!(
            matches!(first.term, BlockPyTerm::Jump(_)),
            "state-changing assignment should still split the block"
        );

        let next = lowered_function
            .blocks
            .iter()
            .find(|block| block.label.contains("__excchk_"))
            .expect("must contain split successor");
        assert_eq!(
            next.body.len(),
            1,
            "ops after the assignment should start a new segment"
        );
    }

    #[test]
    fn preserves_value_return_after_plain_try_except() {
        let source = r#"
def f():
    try:
        pass
    except Exception:
        pass
    return 1
"#;
        let module = transform_str_to_bb_ir_with_options(source, Options::for_test())
            .expect("lowering must succeed")
            .expect("bb module must exist");
        let raw_function = module
            .callable_defs
            .iter()
            .find(|candidate| candidate.names.qualname == "f")
            .expect("must contain raw f");
        assert!(
            raw_function.blocks.iter().any(|block| {
                matches!(
                    block.term,
                    crate::basic_block::block_py::BlockPyTerm::Return(Some(
                        crate::basic_block::block_py::CoreBlockPyExprWithoutAwaitOrYield::Literal(
                            crate::basic_block::block_py::CoreBlockPyLiteral::NumberLiteral(
                                ruff_python_ast::ExprNumberLiteral {
                                    value: ruff_python_ast::Number::Int(_),
                                    ..
                                }
                            )
                        )
                    ))
                )
            }),
            "{raw_function:#?}"
        );
        let lowered = lower_try_jump_exception_flow(&module).expect("pass should succeed");
        let lowered_function = lowered
            .callable_defs
            .iter()
            .find(|candidate| candidate.names.qualname == "f")
            .expect("must contain lowered f");

        assert!(
            lowered_function.blocks.iter().any(|block| {
                matches!(
                    block.term,
                    crate::basic_block::block_py::BlockPyTerm::Return(Some(
                        crate::basic_block::block_py::CoreBlockPyExprWithoutAwaitOrYield::Literal(
                            crate::basic_block::block_py::CoreBlockPyLiteral::NumberLiteral(
                                ruff_python_ast::ExprNumberLiteral {
                                    value: ruff_python_ast::Number::Int(_),
                                    ..
                                }
                            )
                        )
                    ))
                )
            }),
            "{lowered_function:#?}"
        );
    }
}
