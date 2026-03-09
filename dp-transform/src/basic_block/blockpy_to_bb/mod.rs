use super::ast_to_bb::{flatten_stmt, flatten_stmt_boxes, stmt_body_from_stmts};
use super::bb_ir::{BbBlock, BbExpr, BbOp, BbTerm};
use super::block_py::{BlockPyBlock, BlockPyExpr, BlockPyIf, BlockPyLabel, BlockPyStmt};
use crate::transform::ast_rewrite::rewrite_with_pass;
use crate::transform::ast_rewrite::ExprRewritePass;
use crate::transform::context::Context;
use crate::transform::driver::SimplifyExprPass;
use crate::transform::rewrite_stmt;
use ruff_python_ast::{self as ast, Expr, Stmt};
use ruff_text_size::TextRange;
use std::collections::{HashMap, VecDeque};

pub(crate) fn lower_blockpy_blocks_to_bb_blocks(
    context: &Context,
    blocks: &[BlockPyBlock],
    block_params: &HashMap<String, Vec<String>>,
    exception_edges: &HashMap<String, (Option<String>, Option<String>)>,
) -> Vec<BbBlock> {
    let simplify_expr_pass = SimplifyExprPass;
    blocks
        .iter()
        .map(|block| {
            let body_stmts = match block.body.split_last() {
                Some((last, rest)) if is_terminal_blockpy_stmt(last) => rest,
                _ => block.body.as_slice(),
            };
            let mut normalized_body_stmt = stmt_body_from_stmts(
                body_stmts
                    .iter()
                    .filter_map(blockpy_stmt_to_stmt_for_analysis)
                    .collect::<Vec<_>>(),
            );
            rewrite_with_pass(
                context,
                None,
                Some(&simplify_expr_pass),
                &mut normalized_body_stmt,
            );
            let mut normalized_body = flatten_stmt_boxes(&normalized_body_stmt.body)
                .into_iter()
                .map(|stmt| *stmt)
                .collect::<Vec<_>>();
            let mut normalized_term = terminal_stmt_from_blockpy_block(block);
            simplify_blockpy_terminal_exprs(
                context,
                &simplify_expr_pass,
                &mut normalized_term,
                &mut normalized_body,
            );
            let (exc_target_label, exc_name) = exception_edges
                .get(block.label.as_str())
                .cloned()
                .unwrap_or((None, None));
            let mut local_defs = Vec::new();
            let mut ops = Vec::new();
            let mut pending = VecDeque::from(normalized_body);
            while let Some(stmt) = pending.pop_front() {
                match stmt {
                    Stmt::FunctionDef(func_def)
                        if func_def.name.id.as_str().starts_with("_dp_bb_") =>
                    {
                        local_defs.push(func_def);
                    }
                    Stmt::Assign(assign)
                        if rewrite_stmt::assign_del::should_rewrite_targets(&assign.targets) =>
                    {
                        let rewritten = rewrite_stmt::assign_del::rewrite_assign(context, assign);
                        let rewritten_stmt = match rewritten {
                            crate::transform::ast_rewrite::Rewrite::Unmodified(stmt)
                            | crate::transform::ast_rewrite::Rewrite::Walk(stmt) => stmt,
                        };
                        let mut lowered = Vec::new();
                        flatten_stmt(&rewritten_stmt, &mut lowered);
                        for lowered_stmt in lowered.into_iter().rev() {
                            pending.push_front(*lowered_stmt);
                        }
                    }
                    other => {
                        if let Some(op) = BbOp::from_stmt(other) {
                            ops.push(op);
                        }
                    }
                }
            }
            BbBlock {
                label: block.label.as_str().to_string(),
                params: block_params
                    .get(block.label.as_str())
                    .cloned()
                    .unwrap_or_default(),
                local_defs,
                ops,
                exc_target_label,
                exc_name,
                term: bb_term_from_blockpy_terminal_stmt(&normalized_term),
            }
        })
        .collect()
}

fn is_terminal_blockpy_stmt(stmt: &BlockPyStmt) -> bool {
    matches!(
        stmt,
        BlockPyStmt::Jump(_)
            | BlockPyStmt::If(_)
            | BlockPyStmt::BranchTable(_)
            | BlockPyStmt::Raise(_)
            | BlockPyStmt::LegacyTryJump(_)
            | BlockPyStmt::Return(_)
    )
}

fn simplify_expr_for_bb_term(
    context: &Context,
    pass: &SimplifyExprPass,
    expr: &mut Expr,
    body: &mut Vec<Stmt>,
) {
    let lowered = pass.lower_expr(context, expr.clone());
    if lowered.modified {
        let mut lowered_stmts = Vec::new();
        flatten_stmt(&lowered.stmt, &mut lowered_stmts);
        body.extend(lowered_stmts.into_iter().map(|stmt| *stmt));
    }
    *expr = lowered.expr;
}

fn simplify_blockpy_terminal_exprs(
    context: &Context,
    pass: &SimplifyExprPass,
    terminal: &mut BlockPyStmt,
    body: &mut Vec<Stmt>,
) {
    match terminal {
        BlockPyStmt::If(if_stmt) => if_stmt
            .test
            .rewrite_mut(|expr| simplify_expr_for_bb_term(context, pass, expr, body)),
        BlockPyStmt::BranchTable(branch) => branch
            .index
            .rewrite_mut(|expr| simplify_expr_for_bb_term(context, pass, expr, body)),
        BlockPyStmt::Raise(raise_stmt) => {
            if let Some(exc) = raise_stmt.exc.as_mut() {
                exc.rewrite_mut(|expr| simplify_expr_for_bb_term(context, pass, expr, body));
            }
        }
        BlockPyStmt::Return(value) => {
            if let Some(value) = value.as_mut() {
                value.rewrite_mut(|expr| simplify_expr_for_bb_term(context, pass, expr, body));
            }
        }
        BlockPyStmt::Jump(_) | BlockPyStmt::LegacyTryJump(_) => {}
        other => panic!("unsupported terminal BlockPyStmt for simplification: {other:?}"),
    }
}

fn compat_node_index() -> ast::AtomicNodeIndex {
    ast::AtomicNodeIndex::default()
}

fn compat_range() -> TextRange {
    TextRange::default()
}

pub(crate) fn blockpy_stmt_to_stmt_for_analysis(stmt: &BlockPyStmt) -> Option<Stmt> {
    match stmt {
        BlockPyStmt::Pass => Some(Stmt::Pass(ast::StmtPass {
            node_index: compat_node_index(),
            range: compat_range(),
        })),
        BlockPyStmt::Assign(assign) => Some(Stmt::Assign(ast::StmtAssign {
            node_index: compat_node_index(),
            range: compat_range(),
            targets: vec![Expr::Name(assign.target.clone())],
            value: Box::new(assign.value.to_expr()),
        })),
        BlockPyStmt::Expr(expr) => Some(Stmt::Expr(ast::StmtExpr {
            node_index: compat_node_index(),
            range: compat_range(),
            value: Box::new(expr.to_expr()),
        })),
        BlockPyStmt::Delete(delete) => Some(Stmt::Delete(ast::StmtDelete {
            node_index: compat_node_index(),
            range: compat_range(),
            targets: vec![Expr::Name(delete.target.clone())],
        })),
        BlockPyStmt::FunctionDef(func) => Some(Stmt::FunctionDef(func.clone())),
        BlockPyStmt::If(if_stmt) => Some(Stmt::If(ast::StmtIf {
            node_index: compat_node_index(),
            range: compat_range(),
            test: Box::new(if_stmt.test.to_expr()),
            body: stmt_body_from_blockpy_blocks(&if_stmt.body),
            elif_else_clauses: if if_stmt.orelse.is_empty() {
                Vec::new()
            } else {
                vec![ast::ElifElseClause {
                    node_index: compat_node_index(),
                    range: compat_range(),
                    test: None,
                    body: stmt_body_from_blockpy_blocks(&if_stmt.orelse),
                }]
            },
        })),
        BlockPyStmt::Try(try_stmt) => Some(Stmt::Try(ast::StmtTry {
            node_index: compat_node_index(),
            range: compat_range(),
            is_star: try_stmt
                .handlers
                .first()
                .map(|handler| {
                    matches!(
                        handler.kind,
                        super::block_py::BlockPyExceptHandlerKind::ExceptStar
                    )
                })
                .unwrap_or(false),
            body: stmt_body_from_stmts(
                try_stmt
                    .body
                    .iter()
                    .flat_map(|block| block.body.iter())
                    .filter_map(blockpy_stmt_to_stmt_for_analysis)
                    .collect::<Vec<_>>(),
            ),
            handlers: try_stmt
                .handlers
                .iter()
                .map(|handler| {
                    ast::ExceptHandler::ExceptHandler(ast::ExceptHandlerExceptHandler {
                        node_index: compat_node_index(),
                        range: compat_range(),
                        type_: handler.type_.as_ref().map(|expr| Box::new(expr.to_expr())),
                        name: handler
                            .name
                            .as_ref()
                            .map(|name| ast::Identifier::new(name, compat_range())),
                        body: stmt_body_from_stmts(
                            handler
                                .body
                                .iter()
                                .flat_map(|block| block.body.iter())
                                .filter_map(blockpy_stmt_to_stmt_for_analysis)
                                .collect::<Vec<_>>(),
                        ),
                    })
                })
                .collect(),
            orelse: stmt_body_from_stmts(
                try_stmt
                    .orelse
                    .iter()
                    .flat_map(|block| block.body.iter())
                    .filter_map(blockpy_stmt_to_stmt_for_analysis)
                    .collect::<Vec<_>>(),
            ),
            finalbody: stmt_body_from_stmts(
                try_stmt
                    .finalbody
                    .iter()
                    .flat_map(|block| block.body.iter())
                    .filter_map(blockpy_stmt_to_stmt_for_analysis)
                    .collect::<Vec<_>>(),
            ),
        })),
        BlockPyStmt::Jump(_)
        | BlockPyStmt::Return(_)
        | BlockPyStmt::Raise(_)
        | BlockPyStmt::BranchTable(_)
        | BlockPyStmt::LegacyTryJump(_) => None,
    }
}

fn terminal_stmt_from_blockpy_block(block: &BlockPyBlock) -> BlockPyStmt {
    match block.body.last() {
        Some(
            stmt @ (BlockPyStmt::Jump(_)
            | BlockPyStmt::If(_)
            | BlockPyStmt::BranchTable(_)
            | BlockPyStmt::Raise(_)
            | BlockPyStmt::LegacyTryJump(_)
            | BlockPyStmt::Return(_)),
        ) => stmt.clone(),
        Some(other) => panic!("unsupported terminal BlockPyStmt for direct BB lowering: {other:?}"),
        None => BlockPyStmt::Return(None),
    }
}

fn bb_term_from_blockpy_terminal_stmt(terminal: &BlockPyStmt) -> BbTerm {
    match terminal {
        BlockPyStmt::Jump(target) => BbTerm::Jump(target.as_str().to_string()),
        BlockPyStmt::If(if_stmt) => {
            let Some((test, then_label, else_label)) = terminal_if_jump_labels(if_stmt) else {
                panic!("terminal BlockPy If must be `if ...: jump ... else: jump ...`");
            };
            BbTerm::BrIf {
                test: BbExpr::from_expr(test.clone().into()),
                then_label: then_label.as_str().to_string(),
                else_label: else_label.as_str().to_string(),
            }
        }
        BlockPyStmt::BranchTable(branch) => BbTerm::BrTable {
            index: BbExpr::from_expr(branch.index.clone().into()),
            targets: branch
                .targets
                .iter()
                .map(|label| label.as_str().to_string())
                .collect(),
            default_label: branch.default_label.as_str().to_string(),
        },
        BlockPyStmt::Raise(raise_stmt) => BbTerm::Raise {
            exc: raise_stmt
                .exc
                .as_ref()
                .map(|exc| BbExpr::from_expr(exc.clone().into())),
            cause: None,
        },
        BlockPyStmt::LegacyTryJump(try_jump) => BbTerm::TryJump {
            body_label: try_jump.body_label.as_str().to_string(),
            except_label: try_jump.except_label.as_str().to_string(),
            except_exc_name: try_jump.except_exc_name.clone(),
            body_region_labels: try_jump
                .body_region_labels
                .iter()
                .map(|label| label.as_str().to_string())
                .collect(),
            except_region_labels: try_jump
                .except_region_labels
                .iter()
                .map(|label| label.as_str().to_string())
                .collect(),
            finally_label: try_jump
                .finally_label
                .as_ref()
                .map(|label| label.as_str().to_string()),
            finally_exc_name: try_jump.finally_exc_name.clone(),
            finally_region_labels: try_jump
                .finally_region_labels
                .iter()
                .map(|label| label.as_str().to_string())
                .collect(),
            finally_fallthrough_label: try_jump
                .finally_fallthrough_label
                .as_ref()
                .map(|label| label.as_str().to_string()),
        },
        BlockPyStmt::Return(value) => {
            BbTerm::Ret(value.clone().map(|expr| BbExpr::from_expr(expr.into())))
        }
        other => panic!("unsupported terminal BlockPyStmt for direct BbTerm lowering: {other:?}"),
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

fn stmt_body_from_blockpy_blocks(blocks: &[BlockPyBlock]) -> ast::StmtBody {
    stmt_body_from_stmts(
        blocks
            .iter()
            .flat_map(|block| block.body.iter())
            .filter_map(blockpy_stmt_to_stmt_for_analysis)
            .collect::<Vec<_>>(),
    )
}
