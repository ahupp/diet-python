use super::{
    AbruptKind, BlockArg, BlockPyAssign, BlockPyEdge, BlockPyLabel, BlockPyTerm, CfgBlock,
    StructuredBlockPyStmt,
};
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr, ExprName, Stmt};

fn expr_name(id: &str) -> ExprName {
    let Expr::Name(expr) = py_expr!("{id:id}", id = id) else {
        unreachable!();
    };
    expr
}

pub(crate) fn rewrite_region_returns_to_finally_blockpy<E>(
    blocks: &mut [CfgBlock<StructuredBlockPyStmt<E>, BlockPyTerm<E>>],
    finally_target: &BlockPyLabel,
    payload_name: &str,
) where
    E: From<Expr>,
{
    for block in blocks.iter_mut() {
        let ret_value = match std::mem::replace(
            &mut block.term,
            BlockPyTerm::Return(py_expr!("__dp_NONE").into()),
        ) {
            BlockPyTerm::Return(value) => value,
            other => {
                block.term = other;
                continue;
            }
        };
        block
            .body
            .push(StructuredBlockPyStmt::Assign(BlockPyAssign {
                target: expr_name(payload_name),
                value: ret_value,
            }));
        let payload_arg = BlockArg::Name(payload_name.to_string());
        // Only bind the synthetic abrupt slots explicitly. Any ordinary live-ins
        // for the finally entry, including its exception slot, must continue to
        // forward by name once dataflow adds them as block params later.
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
    if matches!(
        func,
        Expr::Name(name) if name.id.as_str() == format!("__dp_{attr_name}")
    ) {
        return true;
    }
    if let Expr::Attribute(attr) = func {
        if attr.attr.as_str() == attr_name {
            if let Expr::Name(module) = attr.value.as_ref() {
                return module.id.as_str() == "__dp__";
            }
        }
    }
    if let Expr::Call(call) = func {
        if !call.arguments.keywords.is_empty() || call.arguments.args.len() != 2 {
            return false;
        }
        if !matches!(
            call.func.as_ref(),
            Expr::Name(name) if name.id.as_str() == "__dp_getattr"
        ) {
            return false;
        }
        let base_matches = matches!(
            &call.arguments.args[0],
            Expr::Name(base) if base.id.as_str() == "__dp__"
        );
        if !base_matches {
            return false;
        }
        return expr_static_str(&call.arguments.args[1]).as_deref() == Some(attr_name);
    }
    false
}

#[cfg(test)]
fn expr_static_str(expr: &Expr) -> Option<String> {
    match expr {
        Expr::StringLiteral(value) => Some(value.value.to_str().to_string()),
        Expr::Call(call)
            if call.arguments.keywords.is_empty()
                && call.arguments.args.len() == 1
                && matches!(
                    call.func.as_ref(),
                    Expr::Name(name)
                        if name.id.as_str() == "__dp_decode_literal_bytes"
                ) =>
        {
            match &call.arguments.args[0] {
                Expr::BytesLiteral(bytes) => {
                    let value: std::borrow::Cow<[u8]> = (&bytes.value).into();
                    String::from_utf8(value.into_owned()).ok()
                }
                _ => None,
            }
        }
        _ => None,
    }
}
