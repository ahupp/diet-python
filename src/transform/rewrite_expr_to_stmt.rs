use super::context::Context;
use crate::template::single_stmt;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt};

pub(crate) enum Modified {
    Yes(Stmt),
    No(Stmt),
}

pub(crate) fn expr_to_stmt(ctx: &Context, stmt: Stmt) -> Modified {
    match stmt {
        Stmt::Assign(assign) => rewrite_assign(ctx, assign),
        Stmt::Expr(expr) => rewrite_expr_stmt(ctx, expr),
        other => Modified::No(other),
    }
}

fn expr_boolop_to_stmts(target: &str, bool_op: ast::ExprBoolOp) -> Vec<Stmt> {
    let ast::ExprBoolOp { op, values, .. } = bool_op;

    let mut values = values.into_iter();
    let first = values.next().expect("bool op expects at least one value");
    let mut stmts = match first {
        Expr::BoolOp(bool_op) => expr_boolop_to_stmts(target, bool_op),
        other => vec![assign_to_target(target, other)],
    };

    for value in values {
        let body_stmt = match value {
            Expr::BoolOp(bool_op) => single_stmt(expr_boolop_to_stmts(target, bool_op)),
            other => assign_to_target(target, other),
        };
        let test_expr = match op {
            ast::BoolOp::And => target_expr(target),
            ast::BoolOp::Or => py_expr!("\nnot {target:expr}", target = target_expr(target),),
        };
        let stmt = py_stmt!(
            "\nif {test:expr}:\n    {body:stmt}",
            test = test_expr,
            body = body_stmt,
        );
        stmts.push(stmt);
    }

    stmts
}

fn assign_to_target(target: &str, value: Expr) -> Stmt {
    py_stmt!(
        "\n{target:id} = {value:expr}",
        target = target,
        value = value,
    )
}

fn target_expr(target: &str) -> Expr {
    py_expr!("\n{target:id}", target = target,)
}

fn rewrite_assign(_ctx: &Context, mut assign: ast::StmtAssign) -> Modified {
    if assign.targets.len() != 1 {
        return Modified::No(Stmt::Assign(assign));
    }

    let Some(Expr::Name(ast::ExprName { id, .. })) = assign.targets.first() else {
        return Modified::No(Stmt::Assign(assign));
    };
    let target_name = id.to_string();

    let value_expr = *assign.value;

    match value_expr {
        Expr::BoolOp(bool_op) => {
            let new_stmt = single_stmt(expr_boolop_to_stmts(&target_name, bool_op));
            Modified::Yes(new_stmt)
        }
        other => {
            assign.value = Box::new(other);
            Modified::No(Stmt::Assign(assign))
        }
    }
}

fn rewrite_expr_stmt(_ctx: &Context, mut expr_stmt: ast::StmtExpr) -> Modified {
    let value_expr = *expr_stmt.value;

    match value_expr {
        Expr::BoolOp(bool_op) => {
            let new_stmt = single_stmt(expr_boolop_to_stmts("_", bool_op));
            Modified::Yes(new_stmt)
        }
        other => {
            expr_stmt.value = Box::new(other);
            Modified::No(Stmt::Expr(expr_stmt))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::context::Context;
    use crate::transform::Options;
    use ruff_python_parser::parse_module;

    fn first_stmt(source: &str) -> Stmt {
        parse_module(source)
            .expect("parse error")
            .into_syntax()
            .body
            .into_iter()
            .next()
            .expect("expected stmt")
    }

    #[test]
    fn rewrites_bool_and_assignment() {
        let stmt = first_stmt("\nx = a and b\n");
        let ctx = Context::new(Options::default());
        match expr_to_stmt(&ctx, stmt) {
            Modified::Yes(Stmt::If(_)) => {}
            _ => panic!("expected bool assignment to be rewritten"),
        }
    }

    #[test]
    fn skips_non_bool_assignment() {
        let stmt = first_stmt("\nx = value\n");
        let ctx = Context::new(Options::default());
        match expr_to_stmt(&ctx, stmt) {
            Modified::No(Stmt::Assign(_)) => {}
            _ => panic!("expected assignment without bool op to be unchanged"),
        }
    }

    #[test]
    fn rewrites_bool_expr_statement() {
        let stmt = first_stmt("\na and b\n");
        let ctx = Context::new(Options::default());
        match expr_to_stmt(&ctx, stmt) {
            Modified::Yes(Stmt::If(_)) => {}
            _ => panic!("expected bool expression statement to be rewritten"),
        }
    }
}
