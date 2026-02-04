use super::*;

fn expr_has_yield(expr: &min_ast::ExprNode) -> bool {
    match expr {
        min_ast::ExprNode::Yield { .. } => true,
        min_ast::ExprNode::Attribute { value, .. } => expr_has_yield(value),
        min_ast::ExprNode::Tuple { elts, .. } => elts.iter().any(expr_has_yield),
        min_ast::ExprNode::Await { value, .. } => expr_has_yield(value),
        min_ast::ExprNode::Call { func, args, .. } => {
            expr_has_yield(func)
                || args.iter().any(|arg| match arg {
                    min_ast::Arg::Positional(expr)
                    | min_ast::Arg::Starred(expr)
                    | min_ast::Arg::KwStarred(expr) => expr_has_yield(expr),
                    min_ast::Arg::Keyword { value, .. } => expr_has_yield(value),
                })
        }
        _ => false,
    }
}

pub(crate) fn stmt_has_yield(stmt: &min_ast::StmtNode) -> bool {
    match stmt {
        min_ast::StmtNode::FunctionDef(_) => false,
        min_ast::StmtNode::While {
            test, body, orelse, ..
        }
        | min_ast::StmtNode::If {
            test, body, orelse, ..
        } => {
            expr_has_yield(test)
                || body.iter().any(stmt_has_yield)
                || orelse.iter().any(stmt_has_yield)
        }
        min_ast::StmtNode::Try {
            body,
            handler,
            orelse,
            finalbody,
            ..
        } => {
            body.iter().any(stmt_has_yield)
                || handler
                    .as_ref()
                    .map(|body| body.iter().any(stmt_has_yield))
                    .unwrap_or(false)
                || orelse.iter().any(stmt_has_yield)
                || finalbody.iter().any(stmt_has_yield)
        }
        min_ast::StmtNode::Raise { exc, .. } => exc.as_ref().map(expr_has_yield).unwrap_or(false),
        min_ast::StmtNode::Return { value, .. } => {
            value.as_ref().map(expr_has_yield).unwrap_or(false)
        }
        min_ast::StmtNode::Expr { value, .. } => expr_has_yield(value),
        min_ast::StmtNode::Assign { value, .. } => expr_has_yield(value),
        min_ast::StmtNode::Delete { .. }
        | min_ast::StmtNode::Break(_)
        | min_ast::StmtNode::Continue(_)
        | min_ast::StmtNode::Pass(_) => false,
    }
}
