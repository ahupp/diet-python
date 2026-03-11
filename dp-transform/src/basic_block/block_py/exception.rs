use super::{BlockPyAssign, BlockPyBlock, BlockPyLabel, BlockPyStmt};
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr, Stmt};

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

fn expr_static_str(expr: &Expr) -> Option<String> {
    match expr {
        Expr::StringLiteral(value) => Some(value.value.to_str().to_string()),
        Expr::Call(call)
            if call.arguments.keywords.is_empty()
                && call.arguments.args.len() == 1
                && matches!(
                    call.func.as_ref(),
                    Expr::Name(name)
                        if matches!(
                            name.id.as_str(),
                            "__dp_decode_literal_bytes" | "__dp_decode_literal_source_bytes"
                        )
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
