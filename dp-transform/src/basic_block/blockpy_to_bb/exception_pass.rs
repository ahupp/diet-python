use crate::basic_block::bb_ir::{BbBlock, BbBlockMeta, BbFunction, BbModule, BbOp, BbTerm};
use std::collections::HashSet;

pub fn lower_try_jump_exception_flow(module: &BbModule) -> Result<BbModule, String> {
    let mut lowered = module.clone();
    for function in lowered.functions_mut() {
        lower_function_try_jump_exception_flow(function)?;
    }
    Ok(lowered)
}

fn lower_function_try_jump_exception_flow(function: &mut BbFunction) -> Result<(), String> {
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

fn split_exception_blocks_for_expr_checks(function: &mut BbFunction) {
    let mut used_labels: HashSet<String> = function
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
        let mut first_local_defs = block.meta.local_defs.clone();
        let mut current_label = block.label.clone();
        let edge_target = block.meta.exc_target_label.clone();
        let edge_exc_name = block.meta.exc_name.clone();
        let mut ops = block.body.into_iter().peekable();
        let mut segment_start_names = known_names.clone();

        let mut segment_ops: Vec<BbOp> = Vec::new();
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
                    term: BbTerm::Jump(next_label.clone()),
                    meta: BbBlockMeta {
                        params: segment_start_names.clone(),
                        local_defs: std::mem::take(&mut first_local_defs),
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
                        local_defs: std::mem::take(&mut first_local_defs),
                        exc_target_label: edge_target.clone(),
                        exc_name: edge_exc_name.clone(),
                    },
                });
            }
        }
    }

    function.blocks = out;
}

fn op_updates_exception_state(op: &BbOp) -> bool {
    matches!(op, BbOp::Assign(_) | BbOp::Delete(_))
}

fn unique_exc_split_label(
    used_labels: &mut HashSet<String>,
    base_label: &str,
    index_seed: usize,
) -> String {
    let mut index = index_seed;
    loop {
        let candidate = format!("{base_label}__excchk_{index}");
        if used_labels.insert(candidate.clone()) {
            return candidate;
        }
        index += 1;
    }
}

fn apply_op_effect_to_known_names(op: &BbOp, known_names: &mut Vec<String>) {
    match op {
        BbOp::Assign(assign) => {
            let target = assign.target.id.to_string();
            if !known_names.iter().any(|name| name == &target) {
                known_names.push(target);
            }
        }
        BbOp::Expr(_) => {}
        BbOp::Delete(delete) => {
            for target in &delete.targets {
                if let crate::basic_block::bb_ir::BbExpr::Name(name) = target {
                    let target_name = name.id.to_string();
                    known_names.retain(|existing| existing != &target_name);
                }
            }
        }
    }
}

fn validate_function_labels(function: &BbFunction, labels: &HashSet<&str>) -> Result<(), String> {
    if !labels.contains(function.entry.as_str()) {
        return Err(format!(
            "missing entry label {} in {}",
            function.entry, function.qualname
        ));
    }
    for block in &function.blocks {
        if let Some(exc_target_label) = block.meta.exc_target_label.as_ref() {
            if !labels.contains(exc_target_label.as_str()) {
                return Err(format!(
                    "unknown exception target {exc_target_label} in {}:{}",
                    function.qualname, block.label
                ));
            }
        }
        match &block.term {
            BbTerm::Jump(target) => {
                ensure_known_label(labels, target, function, &block.label, "jump target")?
            }
            BbTerm::BrIf {
                then_label,
                else_label,
                ..
            } => {
                ensure_known_label(labels, then_label, function, &block.label, "then target")?;
                ensure_known_label(labels, else_label, function, &block.label, "else target")?;
            }
            BbTerm::BrTable {
                targets,
                default_label,
                ..
            } => {
                for target in targets {
                    ensure_known_label(labels, target, function, &block.label, "br_table target")?;
                }
                ensure_known_label(
                    labels,
                    default_label,
                    function,
                    &block.label,
                    "br_table default target",
                )?;
            }
            BbTerm::Raise { .. } | BbTerm::Ret(_) => {}
        }
    }
    Ok(())
}

fn ensure_known_label(
    labels: &HashSet<&str>,
    label: &str,
    function: &BbFunction,
    block_label: &str,
    label_kind: &str,
) -> Result<(), String> {
    if labels.contains(label) {
        return Ok(());
    }
    Err(format!(
        "unknown {label_kind} {label} in {}:{}",
        function.qualname, block_label
    ))
}

#[cfg(test)]
mod tests {
    use super::lower_try_jump_exception_flow;
    use crate::basic_block::bb_ir::{BbBlock, BbBlockMeta, BbTerm};
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
                .functions_mut()
                .iter_mut()
                .find(|function| function.qualname == "f")
                .expect("must contain f");
            let body_label = "_dp_manual_body".to_string();
            let except_label = "_dp_manual_except".to_string();

            function.blocks.push(BbBlock {
                label: body_label.clone(),
                body: vec![],
                term: BbTerm::Ret(None),
                meta: BbBlockMeta {
                    params: vec![],
                    local_defs: vec![],
                    exc_target_label: Some(except_label.clone()),
                    exc_name: Some("_dp_try_exc_manual".to_string()),
                },
            });
            function.blocks.push(BbBlock {
                label: except_label.clone(),
                body: vec![],
                term: BbTerm::Ret(None),
                meta: BbBlockMeta::default(),
            });
            (body_label, except_label)
        };

        let lowered = lower_try_jump_exception_flow(&module).expect("pass should succeed");
        let lowered_function = lowered
            .functions()
            .iter()
            .find(|candidate| candidate.qualname == "f")
            .expect("must contain lowered f");
        let body_block = lowered_function
            .blocks
            .iter()
            .find(|block| block.label == body_label)
            .expect("body block must exist");
        assert_eq!(
            body_block.meta.exc_target_label.as_deref(),
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
            .functions_mut()
            .first_mut()
            .expect("must contain function");
        function.blocks[0].meta.exc_target_label = Some("missing_except".to_string());

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
            .functions_mut()
            .iter_mut()
            .find(|function| function.qualname == "f")
            .expect("must contain f");
        let block_index = function
            .blocks
            .iter()
            .position(|block| block.body.len() >= 2)
            .expect("must contain multi-op block");
        let original_label = function.blocks[block_index].label.clone();
        let except_label = "_dp_manual_except_split".to_string();
        function.blocks.push(BbBlock {
            label: except_label.clone(),
            body: vec![],
            term: BbTerm::Ret(None),
            meta: BbBlockMeta::default(),
        });
        function.blocks[block_index].meta.exc_target_label = Some(except_label.clone());
        function.blocks[block_index].meta.exc_name = Some("_dp_try_exc_split".to_string());

        let lowered = lower_try_jump_exception_flow(&module).expect("pass should succeed");
        let lowered_function = lowered
            .functions()
            .iter()
            .find(|candidate| candidate.qualname == "f")
            .expect("must contain lowered f");

        let first = lowered_function
            .blocks
            .iter()
            .find(|block| block.label == original_label)
            .expect("split must keep original block label");
        assert_eq!(first.body.len(), 1, "first split block must contain one op");
        assert!(
            matches!(first.term, BbTerm::Jump(_)),
            "split op block must jump to next split block"
        );
        assert_eq!(
            first.meta.exc_target_label.as_deref(),
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
            .functions_mut()
            .iter_mut()
            .find(|function| function.qualname == "f")
            .expect("must contain f");
        let block_index = function
            .blocks
            .iter()
            .position(|block| block.body.len() >= 4)
            .expect("must contain multi-op block");
        let original_label = function.blocks[block_index].label.clone();
        let except_label = "_dp_manual_except_group".to_string();
        function.blocks.push(BbBlock {
            label: except_label.clone(),
            body: vec![],
            term: BbTerm::Ret(None),
            meta: BbBlockMeta::default(),
        });
        function.blocks[block_index].meta.exc_target_label = Some(except_label.clone());
        function.blocks[block_index].meta.exc_name = Some("_dp_try_exc_group".to_string());

        let lowered = lower_try_jump_exception_flow(&module).expect("pass should succeed");
        let lowered_function = lowered
            .functions()
            .iter()
            .find(|candidate| candidate.qualname == "f")
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
            matches!(first.term, BbTerm::Jump(_)),
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
            .functions()
            .iter()
            .find(|candidate| candidate.qualname == "f")
            .expect("must contain raw f");
        assert!(
            raw_function.blocks.iter().any(|block| {
                matches!(
                    block.term,
                    crate::basic_block::bb_ir::BbTerm::Ret(Some(
                        crate::basic_block::bb_ir::BbExpr::IntLiteral(_)
                    ))
                )
            }),
            "{raw_function:#?}"
        );
        let lowered = lower_try_jump_exception_flow(&module).expect("pass should succeed");
        let lowered_function = lowered
            .functions()
            .iter()
            .find(|candidate| candidate.qualname == "f")
            .expect("must contain lowered f");

        assert!(
            lowered_function.blocks.iter().any(|block| {
                matches!(
                    block.term,
                    crate::basic_block::bb_ir::BbTerm::Ret(Some(
                        crate::basic_block::bb_ir::BbExpr::IntLiteral(_)
                    ))
                )
            }),
            "{lowered_function:#?}"
        );
    }
}
