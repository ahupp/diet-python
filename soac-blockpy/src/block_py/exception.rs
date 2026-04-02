use super::{
    AbruptKind, BlockArg, BlockPyEdge, BlockPyLabel, BlockPyTerm, CfgBlock, HasMeta,
    ImplicitNoneExpr, Store, StructuredInstr, WithMeta,
};
#[cfg(test)]
use crate::passes::ast_to_ast::util::is_dp_helper_lookup_expr;
use crate::passes::ruff_to_blockpy::RuffToBlockPyExpr;
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr, ExprName, Stmt};

fn expr_name(id: &str) -> ExprName {
    let Expr::Name(expr) = py_expr!("{id:id}", id = id) else {
        unreachable!();
    };
    expr
}

pub(crate) fn rewrite_region_returns_to_finally_blockpy<E>(
    blocks: &mut [CfgBlock<StructuredInstr<E>, BlockPyTerm<E>>],
    finally_target: &BlockPyLabel,
    payload_name: &str,
) where
    E: ImplicitNoneExpr + RuffToBlockPyExpr,
{
    for block in blocks.iter_mut() {
        let ret_value = match std::mem::replace(
            &mut block.term,
            BlockPyTerm::Return(E::implicit_none_expr()),
        ) {
            BlockPyTerm::Return(value) => value,
            other => {
                block.term = other;
                continue;
            }
        };
        let target = expr_name(payload_name);
        let meta = target.meta();
        block.body.push(StructuredInstr::Expr(
            Store::new(target, ret_value).with_meta(meta).into(),
        ));
        let payload_arg = BlockArg::Name(payload_name.to_string());
        // Only bind the synthetic abrupt slots explicitly. The finally entry's
        // current-exception slot continues to forward separately as its declared
        // exception block parameter.
        block.term = BlockPyTerm::Jump(BlockPyEdge::with_args(
            finally_target.clone(),
            vec![BlockArg::AbruptKind(AbruptKind::Return), payload_arg],
        ));
    }
}

pub(crate) fn contains_return_stmt_in_body(stmts: &[Stmt]) -> bool {
    stmts.iter().any(contains_return_stmt)
}

pub(crate) fn contains_return_stmt_in_handlers(handlers: &[ast::ExceptHandler]) -> bool {
    handlers.iter().any(|handler| {
        let ast::ExceptHandler::ExceptHandler(handler) = handler;
        contains_return_stmt_in_body(&handler.body)
    })
}

fn contains_return_stmt(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(_) => true,
        Stmt::If(stmt) => {
            contains_return_stmt_in_body(&stmt.body)
                || stmt
                    .elif_else_clauses
                    .iter()
                    .any(|clause| contains_return_stmt_in_body(&clause.body))
        }
        Stmt::While(stmt) => {
            contains_return_stmt_in_body(&stmt.body) || contains_return_stmt_in_body(&stmt.orelse)
        }
        Stmt::For(stmt) => {
            contains_return_stmt_in_body(&stmt.body) || contains_return_stmt_in_body(&stmt.orelse)
        }
        Stmt::Try(stmt) => {
            contains_return_stmt_in_body(&stmt.body)
                || contains_return_stmt_in_handlers(&stmt.handlers)
                || contains_return_stmt_in_body(&stmt.orelse)
                || contains_return_stmt_in_body(&stmt.finalbody)
        }
        Stmt::With(stmt) => contains_return_stmt_in_body(&stmt.body),
        Stmt::FunctionDef(_) | Stmt::ClassDef(_) => false,
        _ => false,
    }
}

#[cfg(test)]
pub(crate) fn is_dp_lookup_call(func: &Expr, attr_name: &str) -> bool {
    is_dp_helper_lookup_expr(func, attr_name)
}
