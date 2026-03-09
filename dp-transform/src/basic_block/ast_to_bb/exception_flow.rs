use crate::basic_block::block_py::{BlockPyAssign, BlockPyBlock, BlockPyLabel, BlockPyStmt};
use crate::py_expr;
use ruff_python_ast::{self as ast, Stmt};
use std::collections::HashMap;

pub(crate) fn compute_exception_edge_by_label_blockpy(
    blocks: &[BlockPyBlock],
) -> HashMap<String, (Option<String>, Option<String>)> {
    let mut best: HashMap<String, (usize, Option<String>, Option<String>)> = HashMap::new();
    for block in blocks {
        let Some(BlockPyStmt::LegacyTryJump(try_jump)) = block.body.last() else {
            continue;
        };

        let body_rank = try_jump.body_region_labels.len();
        for label in &try_jump.body_region_labels {
            let update = match best.get(label.as_str()) {
                Some((best_rank, _, _)) => body_rank < *best_rank,
                None => true,
            };
            if update {
                best.insert(
                    label.as_str().to_string(),
                    (
                        body_rank,
                        Some(try_jump.except_label.as_str().to_string()),
                        try_jump.except_exc_name.clone(),
                    ),
                );
            }
        }

        if let Some(finally_target) = try_jump.finally_label.as_ref() {
            let except_rank = try_jump.except_region_labels.len();
            for label in &try_jump.except_region_labels {
                let update = match best.get(label.as_str()) {
                    Some((best_rank, _, _)) => except_rank < *best_rank,
                    None => true,
                };
                if update {
                    best.insert(
                        label.as_str().to_string(),
                        (
                            except_rank,
                            Some(finally_target.as_str().to_string()),
                            try_jump.finally_exc_name.clone(),
                        ),
                    );
                }
            }
        }
    }

    best.into_iter()
        .map(|(label, (_, target, exc_name))| (label, (target, exc_name)))
        .collect()
}

pub(crate) fn rewrite_region_returns_to_finally_blockpy(
    blocks: &mut [BlockPyBlock],
    reason_name: &str,
    return_value_name: &str,
    finally_target: &str,
    finally_exc_name: Option<&str>,
) {
    for block in blocks.iter_mut() {
        let ret_value = match block.body.pop() {
            Some(BlockPyStmt::Return(value)) => value,
            Some(stmt) => {
                block.body.push(stmt);
                continue;
            }
            None => continue,
        };
        let ret_expr = ret_value.unwrap_or_else(|| py_expr!("None").into());
        block.body.push(BlockPyStmt::Assign(BlockPyAssign {
            target: ast::ExprName {
                id: reason_name.into(),
                ctx: ast::ExprContext::Store,
                range: Default::default(),
                node_index: ast::AtomicNodeIndex::default(),
            },
            value: py_expr!("'return'").into(),
        }));
        block.body.push(BlockPyStmt::Assign(BlockPyAssign {
            target: ast::ExprName {
                id: return_value_name.into(),
                ctx: ast::ExprContext::Store,
                range: Default::default(),
                node_index: ast::AtomicNodeIndex::default(),
            },
            value: ret_expr.into(),
        }));
        if let Some(finally_exc_name) = finally_exc_name {
            block.body.push(BlockPyStmt::Assign(BlockPyAssign {
                target: ast::ExprName {
                    id: finally_exc_name.into(),
                    ctx: ast::ExprContext::Store,
                    range: Default::default(),
                    node_index: ast::AtomicNodeIndex::default(),
                },
                value: py_expr!("None").into(),
            }));
        }
        block.body.push(BlockPyStmt::Jump(BlockPyLabel::from(
            finally_target.to_string(),
        )));
    }
}

pub(crate) fn contains_return_stmt_in_body(stmts: &[Box<Stmt>]) -> bool {
    stmts.iter().any(|stmt| contains_return_stmt(stmt.as_ref()))
}

pub(crate) fn contains_return_stmt_in_handlers(handlers: &[ast::ExceptHandler]) -> bool {
    handlers.iter().any(|handler| {
        let ast::ExceptHandler::ExceptHandler(handler) = handler;
        contains_return_stmt_in_body(&handler.body.body)
    })
}

fn contains_return_stmt(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(_) => true,
        Stmt::If(stmt) => {
            contains_return_stmt_in_body(&stmt.body.body)
                || stmt
                    .elif_else_clauses
                    .iter()
                    .any(|clause| contains_return_stmt_in_body(&clause.body.body))
        }
        Stmt::While(stmt) => {
            contains_return_stmt_in_body(&stmt.body.body)
                || contains_return_stmt_in_body(&stmt.orelse.body)
        }
        Stmt::For(stmt) => {
            contains_return_stmt_in_body(&stmt.body.body)
                || contains_return_stmt_in_body(&stmt.orelse.body)
        }
        Stmt::Try(stmt) => {
            contains_return_stmt_in_body(&stmt.body.body)
                || contains_return_stmt_in_handlers(&stmt.handlers)
                || contains_return_stmt_in_body(&stmt.orelse.body)
                || contains_return_stmt_in_body(&stmt.finalbody.body)
        }
        Stmt::With(stmt) => contains_return_stmt_in_body(&stmt.body.body),
        Stmt::FunctionDef(_) | Stmt::ClassDef(_) => false,
        _ => false,
    }
}
