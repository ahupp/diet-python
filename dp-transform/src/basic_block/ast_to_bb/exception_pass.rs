use crate::basic_block::bb_ir::{BbBlock, BbFunction, BbModule, BbOp, BbTerm};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
struct RankedExceptionEdge {
    rank: usize,
    target_label: Option<String>,
    exc_name: Option<String>,
}

pub fn lower_try_jump_exception_flow(module: &BbModule) -> Result<BbModule, String> {
    let mut lowered = module.clone();
    for function in &mut lowered.functions {
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

    let computed_edges = compute_exception_edges(function, &label_set)?;
    let qualname = function.qualname.clone();
    for block in &mut function.blocks {
        if let BbTerm::TryJump { body_label, .. } = &block.term {
            block.term = BbTerm::Jump(body_label.clone());
        }
        if let Some((target_label, exc_name)) = computed_edges.get(block.label.as_str()) {
            merge_exception_edge(block, target_label.as_ref(), exc_name.as_ref(), &qualname)?;
        }
    }

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
        if block.exc_target_label.is_none() || block.ops.is_empty() {
            out.push(block);
            continue;
        }

        let mut known_names = block.params.clone();
        let mut first_local_defs = block.local_defs.clone();
        let mut current_label = block.label.clone();
        let edge_target = block.exc_target_label.clone();
        let edge_exc_name = block.exc_name.clone();
        let mut ops = block.ops.into_iter().peekable();

        while let Some(op) = ops.next() {
            let next_label =
                unique_exc_split_label(&mut used_labels, current_label.as_str(), fresh_index);
            fresh_index += 1;

            out.push(BbBlock {
                label: current_label.clone(),
                params: known_names.clone(),
                local_defs: std::mem::take(&mut first_local_defs),
                ops: vec![op.clone()],
                exc_target_label: edge_target.clone(),
                exc_name: edge_exc_name.clone(),
                term: BbTerm::Jump(next_label.clone()),
            });

            apply_op_effect_to_known_names(&op, &mut known_names);
            current_label = next_label;

            if ops.peek().is_none() {
                out.push(BbBlock {
                    label: current_label.clone(),
                    params: known_names.clone(),
                    local_defs: Vec::new(),
                    ops: Vec::new(),
                    exc_target_label: edge_target.clone(),
                    exc_name: edge_exc_name.clone(),
                    term: block.term.clone(),
                });
            }
        }
    }

    function.blocks = out;
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
        if let Some(exc_target_label) = block.exc_target_label.as_ref() {
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
            BbTerm::TryJump {
                body_label,
                except_label,
                body_region_labels,
                except_region_labels,
                finally_label,
                finally_region_labels,
                finally_fallthrough_label,
                ..
            } => {
                ensure_known_label(
                    labels,
                    body_label,
                    function,
                    &block.label,
                    "try body target",
                )?;
                ensure_known_label(
                    labels,
                    except_label,
                    function,
                    &block.label,
                    "try except target",
                )?;
                for label in body_region_labels {
                    ensure_known_label(labels, label, function, &block.label, "try body region")?;
                }
                for label in except_region_labels {
                    ensure_known_label(labels, label, function, &block.label, "try except region")?;
                }
                for label in finally_region_labels {
                    ensure_known_label(
                        labels,
                        label,
                        function,
                        &block.label,
                        "try finally region",
                    )?;
                }
                if let Some(label) = finally_label.as_ref() {
                    ensure_known_label(
                        labels,
                        label,
                        function,
                        &block.label,
                        "try finally target",
                    )?;
                }
                if let Some(label) = finally_fallthrough_label.as_ref() {
                    ensure_known_label(
                        labels,
                        label,
                        function,
                        &block.label,
                        "try finally fallthrough target",
                    )?;
                }
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

fn compute_exception_edges(
    function: &BbFunction,
    labels: &HashSet<&str>,
) -> Result<HashMap<String, (Option<String>, Option<String>)>, String> {
    let mut best: HashMap<String, RankedExceptionEdge> = HashMap::new();
    for block in &function.blocks {
        let BbTerm::TryJump {
            body_region_labels,
            except_region_labels,
            except_label,
            except_exc_name,
            finally_label,
            finally_exc_name,
            ..
        } = &block.term
        else {
            continue;
        };

        ensure_known_label(
            labels,
            except_label,
            function,
            &block.label,
            "try except target",
        )?;
        if let Some(finally_target) = finally_label.as_ref() {
            ensure_known_label(
                labels,
                finally_target,
                function,
                &block.label,
                "try finally target",
            )?;
        }

        let body_rank = body_region_labels.len();
        for label in body_region_labels {
            update_edge_if_better(
                &mut best,
                label.clone(),
                RankedExceptionEdge {
                    rank: body_rank,
                    target_label: Some(except_label.clone()),
                    exc_name: except_exc_name.clone(),
                },
            );
        }

        if let Some(finally_target) = finally_label.as_ref() {
            let except_rank = except_region_labels.len();
            for label in except_region_labels {
                update_edge_if_better(
                    &mut best,
                    label.clone(),
                    RankedExceptionEdge {
                        rank: except_rank,
                        target_label: Some(finally_target.clone()),
                        exc_name: finally_exc_name.clone(),
                    },
                );
            }
        }
    }

    Ok(best
        .into_iter()
        .map(|(label, edge)| (label, (edge.target_label, edge.exc_name)))
        .collect())
}

fn update_edge_if_better(
    best: &mut HashMap<String, RankedExceptionEdge>,
    label: String,
    candidate: RankedExceptionEdge,
) {
    let should_update = match best.get(label.as_str()) {
        Some(existing) => candidate.rank < existing.rank,
        None => true,
    };
    if should_update {
        best.insert(label, candidate);
    }
}

fn merge_exception_edge(
    block: &mut BbBlock,
    computed_target_label: Option<&String>,
    computed_exc_name: Option<&String>,
    qualname: &str,
) -> Result<(), String> {
    if let Some(target_label) = computed_target_label {
        match block.exc_target_label.as_ref() {
            Some(existing) if existing != target_label => {
                return Err(format!(
                    "conflicting exception target for {}:{} (existing={}, computed={})",
                    qualname, block.label, existing, target_label
                ));
            }
            Some(_) => {}
            None => block.exc_target_label = Some(target_label.clone()),
        }
    }

    if let Some(exc_name) = computed_exc_name {
        match block.exc_name.as_ref() {
            Some(existing) if existing != exc_name => {
                return Err(format!(
                    "conflicting exception name for {}:{} (existing={}, computed={})",
                    qualname, block.label, existing, exc_name
                ));
            }
            Some(_) => {}
            None => block.exc_name = Some(exc_name.clone()),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::lower_try_jump_exception_flow;
    use crate::basic_block::bb_ir::{BbBlock, BbTerm};
    use crate::{transform_str_to_bb_ir_with_options, Options};

    #[test]
    fn lowers_try_jump_and_assigns_exception_edges() {
        let source = r#"
def f(x):
    return x
"#;
        let mut module = transform_str_to_bb_ir_with_options(source, Options::for_test())
            .expect("lowering must succeed")
            .expect("bb module must exist");
        let (entry_label, body_label, except_label) = {
            let function = module
                .functions
                .iter_mut()
                .find(|function| function.qualname == "f")
                .expect("must contain f");
            let entry_label = function.entry.clone();
            let start_index = function
                .blocks
                .iter()
                .position(|block| block.label == entry_label)
                .expect("entry block must exist");

            let body_label = "_dp_manual_body".to_string();
            let except_label = "_dp_manual_except".to_string();
            let template = function.blocks[start_index].clone();

            function.blocks.push(BbBlock {
                label: body_label.clone(),
                params: vec![],
                local_defs: vec![],
                ops: vec![],
                exc_target_label: None,
                exc_name: None,
                term: BbTerm::Ret(None),
            });
            function.blocks.push(BbBlock {
                label: except_label.clone(),
                params: vec![],
                local_defs: vec![],
                ops: vec![],
                exc_target_label: None,
                exc_name: None,
                term: BbTerm::Ret(None),
            });

            function.blocks[start_index] = BbBlock {
                label: template.label.clone(),
                params: template.params,
                local_defs: vec![],
                ops: vec![],
                exc_target_label: None,
                exc_name: None,
                term: BbTerm::TryJump {
                    body_label: body_label.clone(),
                    except_label: except_label.clone(),
                    except_exc_name: Some("_dp_try_exc_manual".to_string()),
                    body_region_labels: vec![body_label.clone()],
                    except_region_labels: vec![except_label.clone()],
                    finally_label: None,
                    finally_exc_name: None,
                    finally_region_labels: vec![],
                    finally_fallthrough_label: None,
                },
            };
            (entry_label, body_label, except_label)
        };

        let lowered = lower_try_jump_exception_flow(&module).expect("pass should succeed");
        let lowered_function = lowered
            .functions
            .iter()
            .find(|candidate| candidate.qualname == "f")
            .expect("must contain lowered f");
        let lowered_start = lowered_function
            .blocks
            .iter()
            .find(|block| block.label == entry_label)
            .expect("lowered entry block must exist");

        assert!(
            !matches!(lowered_start.term, BbTerm::TryJump { .. }),
            "try_jump should be lowered"
        );
        assert!(
            matches!(lowered_start.term, BbTerm::Jump(_)),
            "lowered try_jump must become jump"
        );
        let body_block = lowered_function
            .blocks
            .iter()
            .find(|block| block.label == body_label)
            .expect("body block must exist");
        assert_eq!(
            body_block.exc_target_label.as_deref(),
            Some(except_label.as_str()),
            "body region should dispatch to except block on exception"
        );
        assert_eq!(
            body_block.exc_name.as_deref(),
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
        let function = module.functions.first_mut().expect("must contain function");
        function.blocks[0].term = BbTerm::TryJump {
            body_label: "missing_body".to_string(),
            except_label: "missing_except".to_string(),
            except_exc_name: Some("_dp_try_exc_manual".to_string()),
            body_region_labels: vec!["missing_body".to_string()],
            except_region_labels: vec!["missing_except".to_string()],
            finally_label: None,
            finally_exc_name: None,
            finally_region_labels: vec![],
            finally_fallthrough_label: None,
        };

        let err = lower_try_jump_exception_flow(&module).expect_err("must reject unknown labels");
        assert!(
            err.contains("unknown try body target") || err.contains("unknown try except target"),
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
            .functions
            .iter_mut()
            .find(|function| function.qualname == "f")
            .expect("must contain f");
        let block_index = function
            .blocks
            .iter()
            .position(|block| block.ops.len() >= 2)
            .expect("must contain multi-op block");
        let original_label = function.blocks[block_index].label.clone();
        let except_label = "_dp_manual_except_split".to_string();
        function.blocks.push(BbBlock {
            label: except_label.clone(),
            params: vec![],
            local_defs: vec![],
            ops: vec![],
            exc_target_label: None,
            exc_name: None,
            term: BbTerm::Ret(None),
        });
        function.blocks[block_index].exc_target_label = Some(except_label.clone());
        function.blocks[block_index].exc_name = Some("_dp_try_exc_split".to_string());

        let lowered = lower_try_jump_exception_flow(&module).expect("pass should succeed");
        let lowered_function = lowered
            .functions
            .iter()
            .find(|candidate| candidate.qualname == "f")
            .expect("must contain lowered f");

        let first = lowered_function
            .blocks
            .iter()
            .find(|block| block.label == original_label)
            .expect("split must keep original block label");
        assert_eq!(first.ops.len(), 1, "first split block must contain one op");
        assert!(
            matches!(first.term, BbTerm::Jump(_)),
            "split op block must jump to next split block"
        );
        assert_eq!(
            first.exc_target_label.as_deref(),
            Some(except_label.as_str()),
            "split block must preserve exception edge target"
        );

        let split_tail = lowered_function
            .blocks
            .iter()
            .find(|block| block.label.contains("__excchk_"))
            .expect("must contain split tail block");
        assert!(
            split_tail.ops.len() <= 1,
            "split tail block should not aggregate ops"
        );
    }
}
