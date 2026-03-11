use super::{
    BlockPyAssign, BlockPyBlock, BlockPyBranchTable, BlockPyDelete, BlockPyExpr, BlockPyIf,
    BlockPyIfTerm, BlockPyRaise, BlockPyStmt, BlockPyTerm,
};
use crate::basic_block::ast_symbol_analysis::{
    collect_assigned_names, load_names_in_expr, load_names_in_stmt,
};
use crate::transformer::{walk_expr, Transformer};
use ruff_python_ast::{Expr, Stmt};
use std::collections::{HashMap, HashSet};

pub(crate) fn compute_block_params_blockpy(
    blocks: &[BlockPyBlock],
    state_order: &[String],
    extra_successors: &HashMap<String, Vec<String>>,
) -> HashMap<String, Vec<String>> {
    let label_to_index: HashMap<&str, usize> = blocks
        .iter()
        .enumerate()
        .map(|(idx, block)| (block.label.as_str(), idx))
        .collect();
    let analyses: Vec<(HashSet<String>, HashSet<String>)> =
        blocks.iter().map(analyze_blockpy_use_def).collect();
    let mut live_in: Vec<HashSet<String>> = vec![HashSet::new(); blocks.len()];
    let mut live_out: Vec<HashSet<String>> = vec![HashSet::new(); blocks.len()];

    let mut changed = true;
    while changed {
        changed = false;
        for (idx, block) in blocks.iter().enumerate().rev() {
            let mut out = HashSet::new();
            for succ in blockpy_successors(block) {
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
        params.insert(block.label.as_str().to_string(), ordered);
    }
    params
}

pub(crate) fn analyze_blockpy_use_def(block: &BlockPyBlock) -> (HashSet<String>, HashSet<String>) {
    let mut uses = HashSet::new();
    let mut defs = HashSet::new();

    for stmt in &block.body {
        for name in load_names_in_blockpy_stmt(stmt) {
            if !defs.contains(name.as_str()) {
                uses.insert(name);
            }
        }
        for name in assigned_names_in_blockpy_stmt(stmt) {
            defs.insert(name);
        }
    }
    for name in load_names_in_blockpy_term(&block.term) {
        if !defs.contains(name.as_str()) {
            uses.insert(name);
        }
    }

    (uses, defs)
}

fn blockpy_successors(block: &BlockPyBlock) -> Vec<String> {
    match &block.term {
        BlockPyTerm::Jump(target) => vec![target.as_str().to_string()],
        BlockPyTerm::IfTerm(BlockPyIfTerm { body, orelse, .. }) => {
            vec![
                if_term_branch_successor(body),
                if_term_branch_successor(orelse),
            ]
        }
        BlockPyTerm::BranchTable(BlockPyBranchTable {
            targets,
            default_label,
            ..
        }) => {
            let mut out = targets
                .iter()
                .map(|label| label.as_str().to_string())
                .collect::<Vec<_>>();
            out.push(default_label.as_str().to_string());
            out
        }
        BlockPyTerm::TryJump(try_jump) => vec![
            try_jump.body_label.as_str().to_string(),
            try_jump.except_label.as_str().to_string(),
        ],
        BlockPyTerm::Raise(_) | BlockPyTerm::Return(_) => Vec::new(),
    }
}

fn if_term_branch_successor(block: &BlockPyBlock) -> String {
    if block.body.is_empty() {
        if let BlockPyTerm::Jump(target) = &block.term {
            return target.as_str().to_string();
        }
    }
    block.label.as_str().to_string()
}

fn load_names_in_blockpy_stmt(stmt: &BlockPyStmt) -> HashSet<String> {
    match stmt {
        BlockPyStmt::Pass => HashSet::new(),
        BlockPyStmt::Assign(BlockPyAssign { value, .. }) => load_names_in_expr(&value.to_expr()),
        BlockPyStmt::Expr(expr) => load_names_in_expr(&expr.to_expr()),
        BlockPyStmt::Delete(BlockPyDelete { target }) => {
            load_names_in_expr(&Expr::Name(target.clone()))
        }
        BlockPyStmt::FunctionDef(func) => load_names_in_stmt(&Stmt::FunctionDef(func.clone())),
        BlockPyStmt::If(BlockPyIf { test, body, orelse }) => {
            let mut names = load_names_in_expr(&test.to_expr());
            names.extend(load_names_in_blockpy_stmt_list(body));
            names.extend(load_names_in_blockpy_stmt_list(orelse));
            names
        }
        BlockPyStmt::BranchTable(BlockPyBranchTable { index, .. }) => {
            load_names_in_expr(&index.to_expr())
        }
        BlockPyStmt::Jump(_) => HashSet::new(),
        BlockPyStmt::Return(value) => value
            .as_ref()
            .map(|expr| load_names_in_expr(&expr.to_expr()))
            .unwrap_or_default(),
        BlockPyStmt::Raise(BlockPyRaise { exc }) => exc
            .as_ref()
            .map(|expr| load_names_in_expr(&expr.to_expr()))
            .unwrap_or_default(),
        BlockPyStmt::TryJump(_) => HashSet::new(),
    }
}

fn load_names_in_blockpy_term(term: &BlockPyTerm) -> HashSet<String> {
    match term {
        BlockPyTerm::Jump(_) | BlockPyTerm::TryJump(_) => HashSet::new(),
        BlockPyTerm::IfTerm(BlockPyIfTerm { test, body, orelse }) => {
            let mut out = load_names_in_expr(&test.to_expr());
            out.extend(load_names_in_blockpy_stmt_list(&body.body));
            out.extend(load_names_in_blockpy_term(&body.term));
            out.extend(load_names_in_blockpy_stmt_list(&orelse.body));
            out.extend(load_names_in_blockpy_term(&orelse.term));
            out
        }
        BlockPyTerm::BranchTable(BlockPyBranchTable { index, .. }) => {
            load_names_in_expr(&index.to_expr())
        }
        BlockPyTerm::Return(value) => value
            .as_ref()
            .map(|expr| load_names_in_expr(&expr.to_expr()))
            .unwrap_or_default(),
        BlockPyTerm::Raise(BlockPyRaise { exc }) => exc
            .as_ref()
            .map(|expr| load_names_in_expr(&expr.to_expr()))
            .unwrap_or_default(),
    }
}

fn assigned_names_in_blockpy_stmt(stmt: &BlockPyStmt) -> HashSet<String> {
    match stmt {
        BlockPyStmt::Pass => HashSet::new(),
        BlockPyStmt::Assign(BlockPyAssign { target, .. }) => HashSet::from([target.id.to_string()]),
        BlockPyStmt::Expr(_) => HashSet::new(),
        BlockPyStmt::Delete(_) => HashSet::new(),
        BlockPyStmt::FunctionDef(func) => HashSet::from([func.name.id.to_string()]),
        BlockPyStmt::If(BlockPyIf { test, body, orelse }) => {
            let mut names = HashSet::new();
            collect_named_expr_target_names_in_blockpy_expr(test, &mut names);
            for stmt in body {
                names.extend(assigned_names_in_blockpy_stmt(stmt));
            }
            for stmt in orelse {
                names.extend(assigned_names_in_blockpy_stmt(stmt));
            }
            names
        }
        BlockPyStmt::BranchTable(_)
        | BlockPyStmt::Jump(_)
        | BlockPyStmt::Return(_)
        | BlockPyStmt::Raise(_)
        | BlockPyStmt::TryJump(_) => HashSet::new(),
    }
}

fn load_names_in_blockpy_stmt_list(stmts: &[BlockPyStmt]) -> HashSet<String> {
    let mut out = HashSet::new();
    for stmt in stmts {
        out.extend(load_names_in_blockpy_stmt(stmt));
    }
    out
}

fn collect_named_expr_target_names_in_blockpy_expr(
    expr: &BlockPyExpr,
    names: &mut HashSet<String>,
) {
    collect_named_expr_target_names_in_expr(&expr.to_expr(), names);
}

fn collect_named_expr_target_names_in_expr(expr: &Expr, names: &mut HashSet<String>) {
    #[derive(Default)]
    struct NamedExprTargetCollector {
        names: HashSet<String>,
    }

    impl Transformer for NamedExprTargetCollector {
        fn visit_expr(&mut self, expr: &mut Expr) {
            if let Expr::Named(named) = expr {
                collect_assigned_names(named.target.as_ref(), &mut self.names);
                self.visit_expr(named.value.as_mut());
                return;
            }
            walk_expr(self, expr);
        }
    }

    let mut expr = expr.clone();
    let mut collector = NamedExprTargetCollector::default();
    collector.visit_expr(&mut expr);
    names.extend(collector.names);
}
