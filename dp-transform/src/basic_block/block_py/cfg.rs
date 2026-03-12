use super::{BlockPyBlock, BlockPyExpr, BlockPyIfTerm, BlockPyLabel, BlockPyStmt, BlockPyTerm};
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
            for stmt in &mut if_stmt.body {
                rename_blockpy_stmt(stmt, body_renamer, rename, known_labels);
            }
            for stmt in &mut if_stmt.orelse {
                rename_blockpy_stmt(stmt, body_renamer, rename, known_labels);
            }
        }
        BlockPyStmt::BranchTable(branch) => {
            branch
                .index
                .rewrite_mut(|expr| body_renamer.visit_expr(expr));
            for target in &mut branch.targets {
                if let Some(rewritten) = rename.get(target.as_str()) {
                    *target = BlockPyLabel::from(rewritten.clone());
                }
            }
            if let Some(rewritten) = rename.get(branch.default_label.as_str()) {
                branch.default_label = BlockPyLabel::from(rewritten.clone());
            }
        }
        BlockPyStmt::Jump(target) => {
            if let Some(rewritten) = rename.get(target.as_str()) {
                *target = BlockPyLabel::from(rewritten.clone());
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
                }
            }
        }
    }
}

fn rename_blockpy_term(
    term: &mut BlockPyTerm,
    body_renamer: &mut LabelNameRenamer<'_>,
    rename: &HashMap<String, String>,
    known_labels: &HashSet<String>,
) {
    fn rename_target_label(
        label: &mut BlockPyLabel,
        rename: &HashMap<String, String>,
        _known_labels: &HashSet<String>,
        _kind: &str,
    ) {
        if let Some(rewritten) = rename.get(label.as_str()) {
            *label = BlockPyLabel::from(rewritten.clone());
        }
    }

    match term {
        BlockPyTerm::Jump(target) => rename_target_label(target, rename, known_labels, "jump"),
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test,
            then_label,
            else_label,
        }) => {
            test.rewrite_mut(|expr| body_renamer.visit_expr(expr));
            rename_target_label(then_label, rename, known_labels, "if_term then");
            rename_target_label(else_label, rename, known_labels, "if_term else");
        }
        BlockPyTerm::BranchTable(branch) => {
            branch
                .index
                .rewrite_mut(|expr| body_renamer.visit_expr(expr));
            for target in &mut branch.targets {
                rename_target_label(target, rename, known_labels, "br_table");
            }
            rename_target_label(
                &mut branch.default_label,
                rename,
                known_labels,
                "br_table default",
            );
        }
        BlockPyTerm::Raise(raise_stmt) => {
            if let Some(exc) = raise_stmt.exc.as_mut() {
                exc.rewrite_mut(|expr| body_renamer.visit_expr(expr));
            }
        }
        BlockPyTerm::TryJump(try_jump) => {
            rename_target_label(&mut try_jump.body_label, rename, known_labels, "try");
            rename_target_label(&mut try_jump.except_label, rename, known_labels, "try");
        }
        BlockPyTerm::Return(value) => {
            if let Some(value) = value {
                value.rewrite_mut(|expr| body_renamer.visit_expr(expr));
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
    rename_blockpy_term(&mut block.term, body_renamer, rename, known_labels);
}

fn blockpy_successors(block: &BlockPyBlock) -> Vec<String> {
    match &block.term {
        BlockPyTerm::Jump(target) => vec![target.as_str().to_string()],
        BlockPyTerm::IfTerm(if_term) => vec![
            if_term.then_label.as_str().to_string(),
            if_term.else_label.as_str().to_string(),
        ],
        BlockPyTerm::BranchTable(branch) => {
            let mut out = branch
                .targets
                .iter()
                .map(|label| label.as_str().to_string())
                .collect::<Vec<_>>();
            out.push(branch.default_label.as_str().to_string());
            out
        }
        BlockPyTerm::TryJump(try_jump) => vec![
            try_jump.body_label.as_str().to_string(),
            try_jump.except_label.as_str().to_string(),
        ],
        BlockPyTerm::Raise(_) | BlockPyTerm::Return(_) => Vec::new(),
    }
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
                        collect_known_labels_in_stmt_list(&if_stmt.body, out);
                        collect_known_labels_in_stmt_list(&if_stmt.orelse, out);
                    }
                    _ => {}
                }
            }
        }
    }

    fn collect_known_labels_in_stmt_list(stmts: &[BlockPyStmt], out: &mut HashSet<String>) {
        for stmt in stmts {
            if let BlockPyStmt::If(if_stmt) = stmt {
                collect_known_labels_in_stmt_list(&if_stmt.body, out);
                collect_known_labels_in_stmt_list(&if_stmt.orelse, out);
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
        .filter(|block| block.body.is_empty() && matches!(block.term, BlockPyTerm::Return(None)))
        .map(|block| block.label.as_str().to_string())
        .collect();

    for block in blocks.iter_mut() {
        let jump_target = match &block.term {
            BlockPyTerm::Jump(target) => Some(target.as_str().to_string()),
            _ => None,
        };
        if let Some(target) = jump_target {
            if trivial_ret_none_labels.contains(target.as_str()) {
                block.term = BlockPyTerm::Return(None);
            }
        }
    }
}

pub(crate) fn fold_constant_brif_blockpy(blocks: &mut [BlockPyBlock]) {
    for block in blocks.iter_mut() {
        let jump_target = match &block.term {
            BlockPyTerm::IfTerm(BlockPyIfTerm {
                test: BlockPyExpr::BooleanLiteral(boolean),
                then_label,
                else_label,
            }) => {
                if boolean.value {
                    Some(then_label.as_str().to_string())
                } else {
                    Some(else_label.as_str().to_string())
                }
            }
            _ => None,
        };
        if let Some(target) = jump_target {
            block.term = BlockPyTerm::Jump(BlockPyLabel::from(target));
        }
    }
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
