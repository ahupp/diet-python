use crate::block_py::{
    BlockPyBlock, BlockPyBranchTable, BlockPyIf, BlockPyIfTerm, BlockPyRaise, BlockPyStmt,
    BlockPyStmtFragment, BlockPyTerm,
};
use crate::passes::ast_to_ast::ast_rewrite::{Rewrite, StmtRewritePass};
use crate::passes::ast_to_ast::body::suite_mut;
use crate::passes::ast_to_ast::context::Context;
use crate::py_expr;
use crate::transformer::{walk_expr, walk_stmt, Transformer};
use ruff_python_ast::{self as ast, Expr, Stmt};
use std::collections::HashSet;

pub struct SingleNamedAssignmentPass;

fn rewrite_blockpy_expr_deleted_name_loads(
    expr: &mut Expr,
    rewriter: &mut DeletedNameLoadRewriter<'_>,
) {
    rewriter.visit_expr(expr);
}

pub(crate) fn rewrite_deleted_name_loads(
    blocks: &mut [BlockPyBlock<Expr>],
    deleted_names: &HashSet<String>,
    always_unbound_names: &HashSet<String>,
) {
    let mut rewriter = DeletedNameLoadRewriter {
        deleted_names,
        always_unbound_names,
    };
    for block in blocks {
        for stmt in block.body.iter_mut() {
            rewrite_blockpy_stmt_deleted_name_loads(stmt, &mut rewriter);
        }
        rewrite_blockpy_term_deleted_name_loads(&mut block.term, &mut rewriter);
    }
}

fn rewrite_blockpy_stmt_deleted_name_loads(
    stmt: &mut BlockPyStmt<Expr>,
    rewriter: &mut DeletedNameLoadRewriter<'_>,
) {
    match stmt {
        BlockPyStmt::Delete(_) => {}
        BlockPyStmt::Expr(expr) => rewrite_blockpy_expr_deleted_name_loads(expr, rewriter),
        BlockPyStmt::Assign(assign) => {
            rewrite_blockpy_expr_deleted_name_loads(&mut assign.value, rewriter)
        }
        BlockPyStmt::If(BlockPyIf { test, body, orelse }) => {
            rewrite_blockpy_expr_deleted_name_loads(test, rewriter);
            rewrite_blockpy_stmt_fragment_deleted_name_loads(body, rewriter);
            rewrite_blockpy_stmt_fragment_deleted_name_loads(orelse, rewriter);
        }
    }
}

fn rewrite_blockpy_stmt_fragment_deleted_name_loads(
    fragment: &mut BlockPyStmtFragment<Expr>,
    rewriter: &mut DeletedNameLoadRewriter<'_>,
) {
    for stmt in &mut fragment.body {
        rewrite_blockpy_stmt_deleted_name_loads(stmt, rewriter);
    }
    if let Some(term) = &mut fragment.term {
        rewrite_blockpy_term_deleted_name_loads(term, rewriter);
    }
}

fn rewrite_blockpy_term_deleted_name_loads(
    term: &mut BlockPyTerm<Expr>,
    rewriter: &mut DeletedNameLoadRewriter<'_>,
) {
    match term {
        BlockPyTerm::Jump(_) | BlockPyTerm::TryJump(_) => {}
        BlockPyTerm::IfTerm(BlockPyIfTerm { test, .. }) => {
            rewrite_blockpy_expr_deleted_name_loads(test, rewriter);
        }
        BlockPyTerm::BranchTable(BlockPyBranchTable { index, .. }) => {
            rewrite_blockpy_expr_deleted_name_loads(index, rewriter)
        }
        BlockPyTerm::Return(Some(value)) => {
            rewrite_blockpy_expr_deleted_name_loads(value, rewriter)
        }
        BlockPyTerm::Return(None) => {}
        BlockPyTerm::Raise(BlockPyRaise { exc }) => {
            if let Some(exc) = exc {
                rewrite_blockpy_expr_deleted_name_loads(exc, rewriter);
            }
        }
    }
}

struct DeletedNameLoadRewriter<'a> {
    deleted_names: &'a HashSet<String>,
    always_unbound_names: &'a HashSet<String>,
}

impl Transformer for DeletedNameLoadRewriter<'_> {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::FunctionDef(_) | Stmt::ClassDef(_) | Stmt::Delete(_) => {}
            Stmt::Expr(expr_stmt) => self.visit_expr(expr_stmt.value.as_mut()),
            Stmt::Assign(assign) => {
                self.visit_expr(assign.value.as_mut());
            }
            Stmt::AugAssign(aug_assign) => {
                self.visit_expr(aug_assign.target.as_mut());
                self.visit_expr(aug_assign.value.as_mut());
            }
            Stmt::Return(ret) => {
                if let Some(value) = ret.value.as_mut() {
                    self.visit_expr(value.as_mut());
                }
            }
            Stmt::If(if_stmt) => {
                self.visit_expr(if_stmt.test.as_mut());
                self.visit_body(suite_mut(&mut if_stmt.body));
                for clause in if_stmt.elif_else_clauses.iter_mut() {
                    if let Some(test) = clause.test.as_mut() {
                        self.visit_expr(test);
                    }
                    self.visit_body(suite_mut(&mut clause.body));
                }
            }
            Stmt::While(while_stmt) => {
                self.visit_expr(while_stmt.test.as_mut());
                self.visit_body(suite_mut(&mut while_stmt.body));
                self.visit_body(suite_mut(&mut while_stmt.orelse));
            }
            Stmt::For(for_stmt) => {
                self.visit_expr(for_stmt.iter.as_mut());
                self.visit_body(suite_mut(&mut for_stmt.body));
                self.visit_body(suite_mut(&mut for_stmt.orelse));
            }
            Stmt::Try(try_stmt) => {
                self.visit_body(suite_mut(&mut try_stmt.body));
                for handler in try_stmt.handlers.iter_mut() {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    if let Some(type_) = handler.type_.as_mut() {
                        self.visit_expr(type_.as_mut());
                    }
                    self.visit_body(suite_mut(&mut handler.body));
                }
                self.visit_body(suite_mut(&mut try_stmt.orelse));
                self.visit_body(suite_mut(&mut try_stmt.finalbody));
            }
            _ => walk_stmt(self, stmt),
        }
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        if let Expr::Name(name) = expr {
            if matches!(name.ctx, ast::ExprContext::Load) {
                let always_unbound = self.always_unbound_names.contains(name.id.as_str());
                let deleted = self.deleted_names.contains(name.id.as_str());
                if !always_unbound && !deleted {
                    walk_expr(self, expr);
                    return;
                }
                let value = if always_unbound {
                    py_expr!("__dp_DELETED")
                } else {
                    Expr::Name(name.clone())
                };
                let name_value = name.id.to_string();
                *expr = py_expr!(
                    "__dp_load_deleted_name({name:literal}, {value:expr})",
                    name = name_value.as_str(),
                    value = value,
                );
                return;
            }
        }
        walk_expr(self, expr);
    }
}

impl StmtRewritePass for SingleNamedAssignmentPass {
    fn lower_stmt(&self, context: &Context, stmt: Stmt) -> Rewrite {
        match stmt {
            Stmt::Assign(assign) => {
                crate::passes::ruff_to_blockpy::rewrite_assign_stmt(context, assign)
            }
            Stmt::Delete(del) => crate::passes::ruff_to_blockpy::rewrite_delete_stmt(del),
            other => Rewrite::Unmodified(other),
        }
    }
}
