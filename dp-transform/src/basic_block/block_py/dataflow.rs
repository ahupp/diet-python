use super::{
    BlockPyAssign, BlockPyBlock, BlockPyBrIf, BlockPyBranchTable, BlockPyDelete, BlockPyIf,
    BlockPyRaise, BlockPyStmt, BlockPyTerm,
};
use crate::basic_block::ast_symbol_analysis::{
    assigned_names_in_stmt, load_names_in_expr, load_names_in_stmt,
};
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::TextRange;
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
        BlockPyTerm::BrIf(BlockPyBrIf {
            then_label,
            else_label,
            ..
        }) => {
            vec![
                then_label.as_str().to_string(),
                else_label.as_str().to_string(),
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
            load_names_in_stmt(&Stmt::If(ast::StmtIf {
                node_index: compat_node_index(),
                range: compat_range(),
                test: Box::new(test.to_expr()),
                body: stmt_body_from_blockpy_blocks(body),
                elif_else_clauses: if orelse.is_empty() {
                    Vec::new()
                } else {
                    vec![ast::ElifElseClause {
                        node_index: compat_node_index(),
                        range: compat_range(),
                        test: None,
                        body: stmt_body_from_blockpy_blocks(orelse),
                    }]
                },
            }))
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
        BlockPyTerm::BrIf(BlockPyBrIf { test, .. }) => load_names_in_expr(&test.to_expr()),
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
            assigned_names_in_stmt(&Stmt::If(ast::StmtIf {
                node_index: compat_node_index(),
                range: compat_range(),
                test: Box::new(test.to_expr()),
                body: stmt_body_from_blockpy_blocks(body),
                elif_else_clauses: if orelse.is_empty() {
                    Vec::new()
                } else {
                    vec![ast::ElifElseClause {
                        node_index: compat_node_index(),
                        range: compat_range(),
                        test: None,
                        body: stmt_body_from_blockpy_blocks(orelse),
                    }]
                },
            }))
        }
        BlockPyStmt::BranchTable(_)
        | BlockPyStmt::Jump(_)
        | BlockPyStmt::Return(_)
        | BlockPyStmt::Raise(_)
        | BlockPyStmt::TryJump(_) => HashSet::new(),
    }
}

fn compat_node_index() -> ast::AtomicNodeIndex {
    ast::AtomicNodeIndex::default()
}

fn compat_range() -> TextRange {
    TextRange::default()
}

fn stmt_body_from_blockpy_blocks(blocks: &[BlockPyBlock]) -> ast::StmtBody {
    ast::StmtBody {
        node_index: compat_node_index(),
        range: compat_range(),
        body: blocks
            .iter()
            .flat_map(|block| {
                block
                    .body
                    .iter()
                    .filter_map(stmt_from_blockpy_stmt_for_analysis)
                    .chain(stmt_from_blockpy_term_for_analysis(&block.term).into_iter())
            })
            .map(Box::new)
            .collect(),
    }
}

fn stmt_from_blockpy_stmt_for_analysis(stmt: &BlockPyStmt) -> Option<Stmt> {
    match stmt {
        BlockPyStmt::Pass => Some(Stmt::Pass(ast::StmtPass {
            node_index: compat_node_index(),
            range: compat_range(),
        })),
        BlockPyStmt::Assign(BlockPyAssign { target, value }) => {
            Some(Stmt::Assign(ast::StmtAssign {
                node_index: compat_node_index(),
                range: compat_range(),
                targets: vec![Expr::Name(target.clone())],
                value: Box::new(value.to_expr()),
            }))
        }
        BlockPyStmt::Expr(expr) => Some(Stmt::Expr(ast::StmtExpr {
            node_index: compat_node_index(),
            range: compat_range(),
            value: Box::new(expr.to_expr()),
        })),
        BlockPyStmt::Delete(BlockPyDelete { target }) => Some(Stmt::Delete(ast::StmtDelete {
            node_index: compat_node_index(),
            range: compat_range(),
            targets: vec![Expr::Name(target.clone())],
        })),
        BlockPyStmt::FunctionDef(func) => Some(Stmt::FunctionDef(func.clone())),
        BlockPyStmt::If(BlockPyIf { test, body, orelse }) => Some(Stmt::If(ast::StmtIf {
            node_index: compat_node_index(),
            range: compat_range(),
            test: Box::new(test.to_expr()),
            body: stmt_body_from_blockpy_blocks(body),
            elif_else_clauses: if orelse.is_empty() {
                Vec::new()
            } else {
                vec![ast::ElifElseClause {
                    node_index: compat_node_index(),
                    range: compat_range(),
                    test: None,
                    body: stmt_body_from_blockpy_blocks(orelse),
                }]
            },
        })),
        BlockPyStmt::BranchTable(_)
        | BlockPyStmt::Jump(_)
        | BlockPyStmt::Return(_)
        | BlockPyStmt::Raise(_)
        | BlockPyStmt::TryJump(_) => None,
    }
}

fn stmt_from_blockpy_term_for_analysis(term: &BlockPyTerm) -> Option<Stmt> {
    match term {
        BlockPyTerm::Return(value) => Some(Stmt::Return(ast::StmtReturn {
            node_index: compat_node_index(),
            range: compat_range(),
            value: value.clone().map(|value| Box::new(value.to_expr())),
        })),
        BlockPyTerm::Raise(BlockPyRaise { exc }) => Some(Stmt::Raise(ast::StmtRaise {
            node_index: compat_node_index(),
            range: compat_range(),
            exc: exc.clone().map(|exc| Box::new(exc.to_expr())),
            cause: None,
        })),
        BlockPyTerm::Jump(_)
        | BlockPyTerm::BrIf(_)
        | BlockPyTerm::BranchTable(_)
        | BlockPyTerm::TryJump(_) => None,
    }
}
