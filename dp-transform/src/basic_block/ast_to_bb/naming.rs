use super::*;

pub(super) fn sanitize_ident(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

pub(super) fn original_function_name(fn_name: &str) -> String {
    let Some(rest) = fn_name.strip_prefix("_dp_fn_") else {
        return fn_name.to_string();
    };
    let Some((prefix, trailing)) = rest.rsplit_once('_') else {
        return rest.to_string();
    };
    if !trailing.is_empty() && trailing.chars().all(|ch| ch.is_ascii_digit()) {
        prefix.to_string()
    } else {
        rest.to_string()
    }
}

pub(super) fn apply_label_rename(
    entry_label: &str,
    rename: &HashMap<String, String>,
    blocks: &mut [Block],
) -> String {
    let known_labels: HashSet<String> = blocks.iter().map(|block| block.label.clone()).collect();

    for block in blocks.iter_mut() {
        let new_label = rename
            .get(block.label.as_str())
            .cloned()
            .unwrap_or_else(|| block.label.clone());
        block.label = new_label;
        let mut body_renamer = LabelNameRenamer { rename };
        for stmt in block.body.iter_mut() {
            body_renamer.visit_stmt(stmt);
        }
        match &mut block.terminator {
            Terminator::Jump(target) => {
                if let Some(renamed) = rename.get(target.as_str()) {
                    *target = renamed.clone();
                } else if !known_labels.contains(target.as_str()) {
                    panic!("missing renamed jump target: {target}");
                }
            }
            Terminator::BrIf {
                then_label,
                else_label,
                ..
            } => {
                if let Some(renamed) = rename.get(then_label.as_str()) {
                    *then_label = renamed.clone();
                } else if !known_labels.contains(then_label.as_str()) {
                    panic!("missing renamed true target: {then_label}");
                }
                if let Some(renamed) = rename.get(else_label.as_str()) {
                    *else_label = renamed.clone();
                } else if !known_labels.contains(else_label.as_str()) {
                    panic!("missing renamed false target: {else_label}");
                }
            }
            Terminator::BrTable {
                index,
                targets,
                default_label,
            } => {
                body_renamer.visit_expr(index);
                for target in targets.iter_mut() {
                    if let Some(renamed) = rename.get(target.as_str()) {
                        *target = renamed.clone();
                    } else if !known_labels.contains(target.as_str()) {
                        panic!("missing renamed br_table target: {target}");
                    }
                }
                if let Some(renamed) = rename.get(default_label.as_str()) {
                    *default_label = renamed.clone();
                } else if !known_labels.contains(default_label.as_str()) {
                    panic!("missing renamed br_table default target: {default_label}");
                }
            }
            Terminator::Raise(raise_stmt) => {
                if let Some(exc) = raise_stmt.exc.as_mut() {
                    body_renamer.visit_expr(exc.as_mut());
                }
                if let Some(cause) = raise_stmt.cause.as_mut() {
                    body_renamer.visit_expr(cause.as_mut());
                }
            }
            Terminator::TryJump {
                body_label,
                except_label,
                body_region_labels,
                except_region_labels,
                finally_label,
                finally_region_labels,
                finally_fallthrough_label,
                ..
            } => {
                if let Some(renamed) = rename.get(body_label.as_str()) {
                    *body_label = renamed.clone();
                } else if !known_labels.contains(body_label.as_str()) {
                    panic!("missing renamed try body target: {body_label}");
                }
                if let Some(renamed) = rename.get(except_label.as_str()) {
                    *except_label = renamed.clone();
                } else if !known_labels.contains(except_label.as_str()) {
                    panic!("missing renamed except target: {except_label}");
                }
                let mut renamed_body_region = Vec::new();
                for label in body_region_labels.iter() {
                    if let Some(renamed) = rename.get(label.as_str()) {
                        renamed_body_region.push(renamed.clone());
                    } else if known_labels.contains(label.as_str()) {
                        renamed_body_region.push(label.clone());
                    }
                }
                *body_region_labels = renamed_body_region;

                let mut renamed_except_region = Vec::new();
                for label in except_region_labels.iter() {
                    if let Some(renamed) = rename.get(label.as_str()) {
                        renamed_except_region.push(renamed.clone());
                    } else if known_labels.contains(label.as_str()) {
                        renamed_except_region.push(label.clone());
                    }
                }
                *except_region_labels = renamed_except_region;

                if let Some(finally_label_value) = finally_label.as_mut() {
                    if let Some(renamed) = rename.get(finally_label_value.as_str()) {
                        *finally_label_value = renamed.clone();
                    }
                }
                let mut renamed_finally_region = Vec::new();
                for label in finally_region_labels.iter() {
                    if let Some(renamed) = rename.get(label.as_str()) {
                        renamed_finally_region.push(renamed.clone());
                    } else if known_labels.contains(label.as_str()) {
                        renamed_finally_region.push(label.clone());
                    }
                }
                *finally_region_labels = renamed_finally_region;

                if let Some(finally_fallthrough_label_value) = finally_fallthrough_label.as_mut() {
                    if let Some(renamed) = rename.get(finally_fallthrough_label_value.as_str()) {
                        *finally_fallthrough_label_value = renamed.clone();
                    }
                }
            }
            Terminator::Yield { resume_label, .. } => {
                if let Some(renamed) = rename.get(resume_label.as_str()) {
                    *resume_label = renamed.clone();
                } else if !known_labels.contains(resume_label.as_str()) {
                    panic!("missing renamed yield resume target: {resume_label}");
                }
            }
            Terminator::Ret(_) => {}
        }
    }

    rename
        .get(entry_label)
        .cloned()
        .unwrap_or_else(|| entry_label.to_string())
}

pub(super) fn relabel_blocks(prefix: &str, entry_label: &str, blocks: &mut [Block]) -> String {
    let mut rename = HashMap::new();
    rename.insert(entry_label.to_string(), format!("{prefix}_start"));

    let mut next_id = 0usize;
    for block in blocks.iter() {
        if rename.contains_key(block.label.as_str()) {
            continue;
        }
        rename.insert(block.label.clone(), format!("{prefix}_{next_id}"));
        next_id += 1;
    }

    apply_label_rename(entry_label, &rename, blocks)
}

struct LabelNameRenamer<'a> {
    rename: &'a HashMap<String, String>,
}

impl Transformer for LabelNameRenamer<'_> {
    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Name(name) = expr {
            if let Some(rewritten) = self.rename.get(name.id.as_str()) {
                name.id = rewritten.as_str().into();
            }
        }
        walk_expr(self, expr);
    }
}

pub(super) fn fold_jumps_to_trivial_none_return(blocks: &mut [Block]) {
    let trivial_ret_none_labels: HashSet<String> = blocks
        .iter()
        .filter(|block| block.body.is_empty() && matches!(block.terminator, Terminator::Ret(None)))
        .map(|block| block.label.clone())
        .collect();

    for block in blocks.iter_mut() {
        let jump_target = match &block.terminator {
            Terminator::Jump(target) => Some(target.clone()),
            _ => None,
        };
        if let Some(target) = jump_target {
            if trivial_ret_none_labels.contains(target.as_str()) {
                block.terminator = Terminator::Ret(None);
            }
        }
    }
}

pub(super) fn fold_constant_brif(blocks: &mut [Block]) {
    for block in blocks.iter_mut() {
        let jump_target = match &block.terminator {
            Terminator::BrIf {
                test,
                then_label,
                else_label,
            } => match test {
                Expr::BooleanLiteral(boolean) => {
                    if boolean.value {
                        Some(then_label.clone())
                    } else {
                        Some(else_label.clone())
                    }
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(target) = jump_target {
            block.terminator = Terminator::Jump(target);
        }
    }
}

pub(super) fn prune_unreachable_blocks(entry_label: &str, blocks: &mut Vec<Block>) {
    let index_by_label: HashMap<String, usize> = blocks
        .iter()
        .enumerate()
        .map(|(idx, block)| (block.label.clone(), idx))
        .collect();

    let mut worklist = vec![entry_label.to_string()];
    let mut reachable = HashSet::new();
    while let Some(label) = worklist.pop() {
        if !reachable.insert(label.clone()) {
            continue;
        }
        let Some(idx) = index_by_label.get(label.as_str()) else {
            continue;
        };
        for succ in blocks[*idx].successors() {
            worklist.push(succ);
        }
    }

    blocks.retain(|block| reachable.contains(block.label.as_str()));
}
