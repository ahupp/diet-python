use crate::basic_block::block_py::{
    BlockPyBlock, BlockPyBrIf, BlockPyBranchTable, BlockPyIf, BlockPyRaise, BlockPyStmt,
    BlockPyTerm,
};
use crate::transformer::{walk_expr, walk_stmt, Transformer};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt};
use std::collections::HashSet;

pub(crate) fn collect_deleted_names(stmts: &[Box<Stmt>]) -> HashSet<String> {
    let mut names = HashSet::new();
    for stmt in stmts {
        collect_deleted_names_in_stmt(stmt.as_ref(), &mut names);
    }
    names
}

fn collect_deleted_names_in_stmt(stmt: &Stmt, names: &mut HashSet<String>) {
    match stmt {
        Stmt::Delete(delete_stmt) => {
            for target in &delete_stmt.targets {
                collect_deleted_names_in_target(target, names);
            }
        }
        Stmt::If(if_stmt) => {
            for stmt in &if_stmt.body.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
            for clause in &if_stmt.elif_else_clauses {
                for stmt in &clause.body.body {
                    collect_deleted_names_in_stmt(stmt.as_ref(), names);
                }
            }
        }
        Stmt::While(while_stmt) => {
            for stmt in &while_stmt.body.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
            for stmt in &while_stmt.orelse.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
        }
        Stmt::For(for_stmt) => {
            for stmt in &for_stmt.body.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
            for stmt in &for_stmt.orelse.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
        }
        Stmt::Try(try_stmt) => {
            for stmt in &try_stmt.body.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
            for handler in &try_stmt.handlers {
                let ast::ExceptHandler::ExceptHandler(handler) = handler;
                for stmt in &handler.body.body {
                    collect_deleted_names_in_stmt(stmt.as_ref(), names);
                }
            }
            for stmt in &try_stmt.orelse.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
            for stmt in &try_stmt.finalbody.body {
                collect_deleted_names_in_stmt(stmt.as_ref(), names);
            }
        }
        Stmt::FunctionDef(_) | Stmt::ClassDef(_) => {}
        _ => {}
    }
}

fn collect_deleted_names_in_target(target: &Expr, names: &mut HashSet<String>) {
    match target {
        Expr::Name(name) => {
            names.insert(name.id.to_string());
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                collect_deleted_names_in_target(elt, names);
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                collect_deleted_names_in_target(elt, names);
            }
        }
        Expr::Starred(starred) => collect_deleted_names_in_target(starred.value.as_ref(), names),
        _ => {}
    }
}

pub(crate) fn rewrite_delete_to_deleted_sentinel(delete_stmt: &ast::StmtDelete) -> Vec<Stmt> {
    let mut out = Vec::new();
    for target in &delete_stmt.targets {
        rewrite_delete_target_to_deleted_sentinel(target, &mut out);
    }
    out
}

fn rewrite_delete_target_to_deleted_sentinel(target: &Expr, out: &mut Vec<Stmt>) {
    match target {
        Expr::Name(name) => {
            out.push(py_stmt!(
                "{name:id} = __dp_DELETED",
                name = name.id.as_str(),
            ));
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                rewrite_delete_target_to_deleted_sentinel(elt, out);
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                rewrite_delete_target_to_deleted_sentinel(elt, out);
            }
        }
        Expr::Starred(starred) => {
            rewrite_delete_target_to_deleted_sentinel(starred.value.as_ref(), out);
        }
        _ => out.push(py_stmt!("del {target:expr}", target = target.clone())),
    }
}

pub(crate) fn rewrite_deleted_name_loads(
    blocks: &mut [BlockPyBlock],
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
    stmt: &mut BlockPyStmt,
    rewriter: &mut DeletedNameLoadRewriter<'_>,
) {
    match stmt {
        BlockPyStmt::Pass
        | BlockPyStmt::Delete(_)
        | BlockPyStmt::FunctionDef(_)
        | BlockPyStmt::Jump(_)
        | BlockPyStmt::TryJump(_) => {}
        BlockPyStmt::Expr(expr) => expr.rewrite_mut(|inner| rewriter.visit_expr(inner)),
        BlockPyStmt::Assign(assign) => assign.value.rewrite_mut(|expr| rewriter.visit_expr(expr)),
        BlockPyStmt::If(BlockPyIf { test, body, orelse }) => {
            test.rewrite_mut(|expr| rewriter.visit_expr(expr));
            for block in body {
                for stmt in &mut block.body {
                    rewrite_blockpy_stmt_deleted_name_loads(stmt, rewriter);
                }
            }
            for block in orelse {
                for stmt in &mut block.body {
                    rewrite_blockpy_stmt_deleted_name_loads(stmt, rewriter);
                }
            }
        }
        BlockPyStmt::BranchTable(BlockPyBranchTable { index, .. }) => {
            index.rewrite_mut(|expr| rewriter.visit_expr(expr))
        }
        BlockPyStmt::Return(Some(value)) => value.rewrite_mut(|expr| rewriter.visit_expr(expr)),
        BlockPyStmt::Return(None) => {}
        BlockPyStmt::Raise(BlockPyRaise { exc }) => {
            if let Some(exc) = exc {
                exc.rewrite_mut(|expr| rewriter.visit_expr(expr));
            }
        }
    }
}

fn rewrite_blockpy_term_deleted_name_loads(
    term: &mut BlockPyTerm,
    rewriter: &mut DeletedNameLoadRewriter<'_>,
) {
    match term {
        BlockPyTerm::Jump(_) | BlockPyTerm::TryJump(_) => {}
        BlockPyTerm::BrIf(BlockPyBrIf { test, .. }) => {
            test.rewrite_mut(|expr| rewriter.visit_expr(expr))
        }
        BlockPyTerm::BranchTable(BlockPyBranchTable { index, .. }) => {
            index.rewrite_mut(|expr| rewriter.visit_expr(expr))
        }
        BlockPyTerm::Return(Some(value)) => value.rewrite_mut(|expr| rewriter.visit_expr(expr)),
        BlockPyTerm::Return(None) => {}
        BlockPyTerm::Raise(BlockPyRaise { exc }) => {
            if let Some(exc) = exc {
                exc.rewrite_mut(|expr| rewriter.visit_expr(expr));
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
            Stmt::Raise(raise_stmt) => {
                if let Some(exc) = raise_stmt.exc.as_mut() {
                    self.visit_expr(exc.as_mut());
                }
                if let Some(cause) = raise_stmt.cause.as_mut() {
                    self.visit_expr(cause.as_mut());
                }
            }
            Stmt::If(if_stmt) => {
                self.visit_expr(if_stmt.test.as_mut());
                self.visit_body(&mut if_stmt.body);
                for clause in if_stmt.elif_else_clauses.iter_mut() {
                    if let Some(test) = clause.test.as_mut() {
                        self.visit_expr(test);
                    }
                    self.visit_body(&mut clause.body);
                }
            }
            Stmt::While(while_stmt) => {
                self.visit_expr(while_stmt.test.as_mut());
                self.visit_body(&mut while_stmt.body);
                self.visit_body(&mut while_stmt.orelse);
            }
            Stmt::For(for_stmt) => {
                self.visit_expr(for_stmt.iter.as_mut());
                self.visit_body(&mut for_stmt.body);
                self.visit_body(&mut for_stmt.orelse);
            }
            Stmt::Try(try_stmt) => {
                self.visit_body(&mut try_stmt.body);
                for handler in try_stmt.handlers.iter_mut() {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    if let Some(type_) = handler.type_.as_mut() {
                        self.visit_expr(type_.as_mut());
                    }
                    self.visit_body(&mut handler.body);
                }
                self.visit_body(&mut try_stmt.orelse);
                self.visit_body(&mut try_stmt.finalbody);
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
