use super::{Block, Terminator};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Stmt};
use std::collections::{HashMap, HashSet};

pub(super) fn compute_exception_edge_by_label(
    blocks: &[Block],
) -> HashMap<String, (Option<String>, Option<String>)> {
    let mut best: HashMap<String, (usize, Option<String>, Option<String>)> = HashMap::new();
    for block in blocks {
        let Terminator::TryJump {
            body_region_labels,
            except_region_labels,
            except_label,
            except_exc_name,
            finally_label,
            finally_exc_name,
            ..
        } = &block.terminator
        else {
            continue;
        };

        let body_rank = body_region_labels.len();
        for label in body_region_labels {
            let update = match best.get(label.as_str()) {
                Some((best_rank, _, _)) => body_rank < *best_rank,
                None => true,
            };
            if update {
                best.insert(
                    label.clone(),
                    (
                        body_rank,
                        Some(except_label.clone()),
                        except_exc_name.clone(),
                    ),
                );
            }
        }

        if let Some(finally_target) = finally_label.as_ref() {
            let except_rank = except_region_labels.len();
            for label in except_region_labels {
                let update = match best.get(label.as_str()) {
                    Some((best_rank, _, _)) => except_rank < *best_rank,
                    None => true,
                };
                if update {
                    best.insert(
                        label.clone(),
                        (
                            except_rank,
                            Some(finally_target.clone()),
                            finally_exc_name.clone(),
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

pub(super) fn rewrite_region_returns_to_finally(
    blocks: &mut [Block],
    region_labels: &[String],
    reason_name: &str,
    return_value_name: &str,
    finally_target: &str,
    finally_exc_name: Option<&str>,
) {
    let region: HashSet<&str> = region_labels.iter().map(|label| label.as_str()).collect();
    for block in blocks.iter_mut() {
        if !region.contains(block.label.as_str()) {
            continue;
        }
        let ret_value = match &block.terminator {
            Terminator::Ret(value) => value.clone(),
            _ => continue,
        };
        let ret_expr = ret_value.unwrap_or_else(|| py_expr!("None"));
        block
            .body
            .push(py_stmt!("{name:id} = 'return'", name = reason_name,));
        block.body.push(py_stmt!(
            "{name:id} = {value:expr}",
            name = return_value_name,
            value = ret_expr,
        ));
        if let Some(finally_exc_name) = finally_exc_name {
            block
                .body
                .push(py_stmt!("{name:id} = None", name = finally_exc_name,));
        }
        block.terminator = Terminator::Jump(finally_target.to_string());
    }
}

pub(super) fn contains_return_stmt_in_body(stmts: &[Box<Stmt>]) -> bool {
    stmts.iter().any(|stmt| contains_return_stmt(stmt.as_ref()))
}

pub(super) fn contains_return_stmt_in_handlers(handlers: &[ast::ExceptHandler]) -> bool {
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
