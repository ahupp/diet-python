use super::{BlockPyBlock, BlockPyExpr, BlockPyIf, BlockPyLabel, BlockPyStmt};
use crate::transformer::{walk_expr, Transformer};
use ruff_python_ast::{Expr, Stmt};
use std::collections::{HashMap, HashSet};

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

fn rename_blockpy_stmt(
    stmt: &mut BlockPyStmt,
    body_renamer: &mut LabelNameRenamer<'_>,
    rename: &HashMap<String, String>,
    known_labels: &HashSet<String>,
) {
    match stmt {
        BlockPyStmt::Pass => {}
        BlockPyStmt::Assign(assign) => {
            if let Some(rewritten) = rename.get(assign.target.id.as_str()) {
                assign.target.id = rewritten.as_str().into();
            }
            assign
                .value
                .rewrite_mut(|expr| body_renamer.visit_expr(expr));
        }
        BlockPyStmt::Expr(expr) => expr.rewrite_mut(|inner| body_renamer.visit_expr(inner)),
        BlockPyStmt::Delete(delete) => {
            if let Some(rewritten) = rename.get(delete.target.id.as_str()) {
                delete.target.id = rewritten.as_str().into();
            }
        }
        BlockPyStmt::FunctionDef(func) => {
            let mut stmt = Stmt::FunctionDef(func.clone());
            body_renamer.visit_stmt(&mut stmt);
            let Stmt::FunctionDef(rewritten) = stmt else {
                unreachable!("function def stayed a function def")
            };
            *func = rewritten;
        }
        BlockPyStmt::If(if_stmt) => {
            if_stmt
                .test
                .rewrite_mut(|expr| body_renamer.visit_expr(expr));
            for block in &mut if_stmt.body {
                for stmt in &mut block.body {
                    rename_blockpy_stmt(stmt, body_renamer, rename, known_labels);
                }
            }
            for block in &mut if_stmt.orelse {
                for stmt in &mut block.body {
                    rename_blockpy_stmt(stmt, body_renamer, rename, known_labels);
                }
            }
        }
        BlockPyStmt::BranchTable(branch) => {
            branch
                .index
                .rewrite_mut(|expr| body_renamer.visit_expr(expr));
            for target in &mut branch.targets {
                if let Some(rewritten) = rename.get(target.as_str()) {
                    *target = BlockPyLabel::from(rewritten.clone());
                } else if !known_labels.contains(target.as_str()) {
                    panic!("missing renamed br_table target: {}", target.as_str());
                }
            }
            if let Some(rewritten) = rename.get(branch.default_label.as_str()) {
                branch.default_label = BlockPyLabel::from(rewritten.clone());
            } else if !known_labels.contains(branch.default_label.as_str()) {
                panic!(
                    "missing renamed br_table default target: {}",
                    branch.default_label.as_str()
                );
            }
        }
        BlockPyStmt::Jump(target) => {
            if let Some(rewritten) = rename.get(target.as_str()) {
                *target = BlockPyLabel::from(rewritten.clone());
            } else if !known_labels.contains(target.as_str()) {
                panic!("missing renamed jump target: {}", target.as_str());
            }
        }
        BlockPyStmt::Return(value) => {
            if let Some(value) = value {
                value.rewrite_mut(|expr| body_renamer.visit_expr(expr));
            }
        }
        BlockPyStmt::Raise(raise_stmt) => {
            if let Some(exc) = raise_stmt.exc.as_mut() {
                exc.rewrite_mut(|expr| body_renamer.visit_expr(expr));
            }
        }
        BlockPyStmt::TryJump(try_jump) => {
            for label in [&mut try_jump.body_label, &mut try_jump.except_label] {
                if let Some(rewritten) = rename.get(label.as_str()) {
                    *label = BlockPyLabel::from(rewritten.clone());
                } else if !known_labels.contains(label.as_str()) {
                    panic!("missing renamed try target: {}", label.as_str());
                }
            }
        }
    }
}

fn rename_blockpy_block(
    block: &mut BlockPyBlock,
    body_renamer: &mut LabelNameRenamer<'_>,
    rename: &HashMap<String, String>,
    known_labels: &HashSet<String>,
) {
    let new_label = rename
        .get(block.label.as_str())
        .cloned()
        .unwrap_or_else(|| block.label.as_str().to_string());
    block.label = BlockPyLabel::from(new_label);
    for stmt in &mut block.body {
        rename_blockpy_stmt(stmt, body_renamer, rename, known_labels);
    }
}

fn blockpy_successors(block: &BlockPyBlock) -> Vec<String> {
    fn collect_stmt_successors(stmt: &BlockPyStmt, out: &mut Vec<String>) {
        match stmt {
            BlockPyStmt::Jump(target) => out.push(target.as_str().to_string()),
            BlockPyStmt::If(if_stmt) => {
                if let Some((_, then_label, else_label)) = terminal_if_jump_labels(if_stmt) {
                    out.push(then_label.as_str().to_string());
                    out.push(else_label.as_str().to_string());
                }
                for block in if_stmt.body.iter().chain(if_stmt.orelse.iter()) {
                    for stmt in &block.body {
                        collect_stmt_successors(stmt, out);
                    }
                }
            }
            BlockPyStmt::BranchTable(branch) => {
                out.extend(
                    branch
                        .targets
                        .iter()
                        .map(|label| label.as_str().to_string()),
                );
                out.push(branch.default_label.as_str().to_string());
            }
            BlockPyStmt::TryJump(try_jump) => {
                out.push(try_jump.body_label.as_str().to_string());
                out.push(try_jump.except_label.as_str().to_string());
            }
            _ => {}
        }
    }

    let mut out = Vec::new();
    for stmt in &block.body {
        collect_stmt_successors(stmt, &mut out);
    }
    out
}

fn apply_label_rename_blockpy(
    entry_label: &str,
    rename: &HashMap<String, String>,
    blocks: &mut [BlockPyBlock],
) -> String {
    fn collect_known_labels(blocks: &[BlockPyBlock], out: &mut HashSet<String>) {
        for block in blocks {
            out.insert(block.label.as_str().to_string());
            for stmt in &block.body {
                match stmt {
                    BlockPyStmt::If(if_stmt) => {
                        collect_known_labels(&if_stmt.body, out);
                        collect_known_labels(&if_stmt.orelse, out);
                    }
                    _ => {}
                }
            }
        }
    }

    let mut known_labels = HashSet::new();
    collect_known_labels(blocks, &mut known_labels);
    for block in blocks.iter_mut() {
        let mut body_renamer = LabelNameRenamer { rename };
        rename_blockpy_block(block, &mut body_renamer, rename, &known_labels);
    }
    rename
        .get(entry_label)
        .cloned()
        .unwrap_or_else(|| entry_label.to_string())
}

pub(crate) fn relabel_blockpy_blocks(
    prefix: &str,
    entry_label: &str,
    blocks: &mut [BlockPyBlock],
) -> (String, HashMap<String, String>) {
    let mut rename = HashMap::new();
    rename.insert(entry_label.to_string(), format!("{prefix}_start"));

    let mut next_id = 0usize;
    for block in blocks.iter() {
        if rename.contains_key(block.label.as_str()) {
            continue;
        }
        rename.insert(
            block.label.as_str().to_string(),
            format!("{prefix}_{next_id}"),
        );
        next_id += 1;
    }

    let rewritten_entry = apply_label_rename_blockpy(entry_label, &rename, blocks);
    (rewritten_entry, rename)
}

pub(crate) fn fold_jumps_to_trivial_none_return_blockpy(blocks: &mut [BlockPyBlock]) {
    let trivial_ret_none_labels: HashSet<String> = blocks
        .iter()
        .filter(|block| {
            block.body.len() == 1 && matches!(block.body.last(), Some(BlockPyStmt::Return(None)))
        })
        .map(|block| block.label.as_str().to_string())
        .collect();

    for block in blocks.iter_mut() {
        let jump_target = match block.body.last() {
            Some(BlockPyStmt::Jump(target)) => Some(target.as_str().to_string()),
            _ => None,
        };
        if let Some(target) = jump_target {
            if trivial_ret_none_labels.contains(target.as_str()) {
                if let Some(last) = block.body.last_mut() {
                    *last = BlockPyStmt::Return(None);
                }
            }
        }
    }
}

pub(crate) fn fold_constant_brif_blockpy(blocks: &mut [BlockPyBlock]) {
    for block in blocks.iter_mut() {
        let jump_target = match block.body.last() {
            Some(BlockPyStmt::If(if_stmt)) => match terminal_if_jump_labels(if_stmt) {
                Some((BlockPyExpr::BooleanLiteral(boolean), then_label, else_label)) => {
                    if boolean.value {
                        Some(then_label.as_str().to_string())
                    } else {
                        Some(else_label.as_str().to_string())
                    }
                }
                _ => None,
            },
            _ => None,
        };
        if let Some(target) = jump_target {
            if let Some(last) = block.body.last_mut() {
                *last = BlockPyStmt::Jump(BlockPyLabel::from(target));
            }
        }
    }
}

fn terminal_if_jump_labels(
    if_stmt: &BlockPyIf,
) -> Option<(&BlockPyExpr, &BlockPyLabel, &BlockPyLabel)> {
    let [BlockPyBlock {
        body: then_body, ..
    }] = if_stmt.body.as_slice()
    else {
        return None;
    };
    let [BlockPyStmt::Jump(then_label)] = then_body.as_slice() else {
        return None;
    };
    let [BlockPyBlock {
        body: else_body, ..
    }] = if_stmt.orelse.as_slice()
    else {
        return None;
    };
    let [BlockPyStmt::Jump(else_label)] = else_body.as_slice() else {
        return None;
    };
    Some((&if_stmt.test, then_label, else_label))
}

pub(crate) fn prune_unreachable_blockpy_blocks(
    entry_label: &str,
    extra_roots: &[String],
    blocks: &mut Vec<BlockPyBlock>,
) {
    let index_by_label: HashMap<String, usize> = blocks
        .iter()
        .enumerate()
        .map(|(idx, block)| (block.label.as_str().to_string(), idx))
        .collect();

    let mut worklist = vec![entry_label.to_string()];
    worklist.extend(extra_roots.iter().cloned());
    let mut reachable = HashSet::new();
    while let Some(label) = worklist.pop() {
        if !reachable.insert(label.clone()) {
            continue;
        }
        let Some(idx) = index_by_label.get(label.as_str()) else {
            continue;
        };
        for succ in blockpy_successors(&blocks[*idx]) {
            worklist.push(succ);
        }
    }

    blocks.retain(|block| reachable.contains(block.label.as_str()));
}
