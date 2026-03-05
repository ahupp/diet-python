use super::{Block, Terminator};
use super::symbol_analysis::{assigned_names_in_stmt, load_names_in_stmt, load_names_in_terminator};
use std::collections::{HashMap, HashSet};

pub(super) fn build_extra_successors(blocks: &[Block]) -> HashMap<String, Vec<String>> {
    let mut extra = HashMap::new();
    for block in blocks {
        if let Terminator::TryJump {
            body_region_labels,
            except_region_labels,
            finally_label: Some(finally_label),
            ..
        } = &block.terminator
        {
            for label in body_region_labels.iter().chain(except_region_labels.iter()) {
                extra
                    .entry(label.clone())
                    .or_insert_with(Vec::new)
                    .push(finally_label.clone());
            }
        }
    }
    extra
}

pub(super) fn compute_block_params(
    blocks: &[Block],
    state_order: &[String],
    extra_successors: &HashMap<String, Vec<String>>,
) -> HashMap<String, Vec<String>> {
    let label_to_index: HashMap<&str, usize> = blocks
        .iter()
        .enumerate()
        .map(|(idx, block)| (block.label.as_str(), idx))
        .collect();
    let analyses: Vec<(HashSet<String>, HashSet<String>)> =
        blocks.iter().map(analyze_block_use_def).collect();
    let mut live_in: Vec<HashSet<String>> = vec![HashSet::new(); blocks.len()];
    let mut live_out: Vec<HashSet<String>> = vec![HashSet::new(); blocks.len()];

    let mut changed = true;
    while changed {
        changed = false;
        for (idx, block) in blocks.iter().enumerate().rev() {
            let mut out = HashSet::new();
            for succ in block.successors() {
                if let Some(succ_idx) = label_to_index.get(succ.as_str()) {
                    out.extend(live_in[*succ_idx].iter().cloned());
                }
            }
            if let Some(extra) = extra_successors.get(block.label.as_str()) {
                for succ in extra {
                    if let Some(succ_idx) = label_to_index.get(succ.as_str()) {
                        out.extend(live_in[*succ_idx].iter().cloned());
                    }
                }
            }
            let (uses, defs) = &analyses[idx];
            let mut incoming = uses.clone();
            for name in &out {
                if !defs.contains(name) {
                    incoming.insert(name.clone());
                }
            }
            if incoming != live_in[idx] || out != live_out[idx] {
                changed = true;
                live_in[idx] = incoming;
                live_out[idx] = out;
            }
        }
    }

    let mut params = HashMap::new();
    for (idx, block) in blocks.iter().enumerate() {
        let ordered = state_order
            .iter()
            .filter(|name| live_in[idx].contains(name.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        params.insert(block.label.clone(), ordered);
    }
    params
}

pub(super) fn ensure_try_exception_params(
    blocks: &[Block],
    block_params: &mut HashMap<String, Vec<String>>,
) {
    for block in blocks {
        let Terminator::TryJump {
            except_label,
            except_exc_name,
            finally_label,
            finally_exc_name,
            ..
        } = &block.terminator
        else {
            continue;
        };

        if let Some(exc_name) = except_exc_name {
            let params = block_params.entry(except_label.clone()).or_default();
            params.retain(|name| name != exc_name);
            params.push(exc_name.clone());
        }
        if let (Some(finally_label), Some(exc_name)) = (finally_label, finally_exc_name) {
            let params = block_params.entry(finally_label.clone()).or_default();
            params.retain(|name| name != exc_name);
            params.push(exc_name.clone());
        }
    }
}

pub(super) fn analyze_block_use_def(block: &Block) -> (HashSet<String>, HashSet<String>) {
    let mut uses = HashSet::new();
    let mut defs = HashSet::new();

    for stmt in &block.body {
        for name in load_names_in_stmt(stmt) {
            if !defs.contains(name.as_str()) {
                uses.insert(name);
            }
        }
        for name in assigned_names_in_stmt(stmt) {
            defs.insert(name);
        }
    }

    for name in assigned_names_in_terminator(&block.terminator) {
        defs.insert(name);
    }

    for name in load_names_in_terminator(&block.terminator) {
        if !defs.contains(name.as_str()) {
            uses.insert(name);
        }
    }

    (uses, defs)
}

fn assigned_names_in_terminator(terminator: &Terminator) -> HashSet<String> {
    match terminator {
        Terminator::Jump(_)
        | Terminator::BrIf { .. }
        | Terminator::BrTable { .. }
        | Terminator::Raise(_)
        | Terminator::Yield { .. }
        | Terminator::Ret(_) => HashSet::new(),
        Terminator::TryJump {
            except_exc_name,
            finally_exc_name,
            ..
        } => {
            let mut names = HashSet::new();
            if let Some(name) = except_exc_name.as_ref() {
                names.insert(name.clone());
            }
            if let Some(name) = finally_exc_name.as_ref() {
                names.insert(name.clone());
            }
            names
        }
    }
}
