use crate::template::{make_binop, make_unaryop};
use crate::transform::context::Context;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, CmpOp, Expr, Stmt};

pub(crate) fn expr_boolop_to_stmts(target: &str, bool_op: ast::ExprBoolOp) -> Vec<Stmt> {
    let ast::ExprBoolOp { op, values, .. } = bool_op;

    let mut values = values.into_iter();
    let first = values.next().expect("bool op expects at least one value");
    let mut stmts = match first {
        Expr::BoolOp(bool_op) => expr_boolop_to_stmts(target, bool_op),
        other => py_stmt!("{target:id} = {value:expr}", target = target, value = other),
    };

    for value in values {
        let body_stmt = match value {
            Expr::BoolOp(bool_op) => expr_boolop_to_stmts(target, bool_op),
            other => py_stmt!("{target:id} = {value:expr}", target = target, value = other),
        };
        let test_expr = match op {
            ast::BoolOp::And => target_expr(target),
            ast::BoolOp::Or => py_expr!("not {target:expr}", target = target_expr(target),),
        };
        let stmt = py_stmt!(
            r#"
if {test:expr}:
    {body:stmt}
"#,
            test = test_expr,
            body = body_stmt,
        );
        stmts.extend(stmt);
    }

    stmts
}

pub(crate) fn expr_compare_to_stmts(
    ctx: &Context,
    target: &str,
    compare: ast::ExprCompare,
) -> Vec<Stmt> {
    let ast::ExprCompare {
        left,
        ops,
        comparators,
        ..
    } = compare;

    let ops = ops.into_vec();
    let comparators = comparators.into_vec();
    let count = ops.len();

    let mut stmts = Vec::new();
    let mut current_left = *left;

    for (index, (op, comparator)) in ops.into_iter().zip(comparators.into_iter()).enumerate() {
        let mut comparator_expr = comparator;
        let mut prelude = Vec::new();
        if index < count - 1 {
            let tmp = ctx.fresh("compare");
            prelude.extend(py_stmt!(
                "{tmp:id} = {value:expr}",
                tmp = tmp.as_str(),
                value = comparator_expr.clone(),
            ));
            comparator_expr = py_expr!("{tmp:id}", tmp = tmp.as_str());
        }

        let comparison = compare_expr(op, current_left.clone(), comparator_expr.clone());

        if index == 0 {
            stmts.extend(prelude);
            stmts.extend(assign_to_target(target, comparison));
        } else {
            let mut body = prelude;
            body.extend(assign_to_target(target, comparison));
            let stmt = py_stmt!(
                r#"
if {test:expr}:
    {body:stmt}
"#,
                test = target_expr(target),
                body = body,
            );
            stmts.extend(stmt);
        }

        current_left = comparator_expr;
    }

    stmts
}

fn compare_expr(op: CmpOp, left: Expr, right: Expr) -> Expr {
    match op {
        CmpOp::Eq => make_binop("eq", left, right),
        CmpOp::NotEq => make_binop("ne", left, right),
        CmpOp::Lt => make_binop("lt", left, right),
        CmpOp::LtE => make_binop("le", left, right),
        CmpOp::Gt => make_binop("gt", left, right),
        CmpOp::GtE => make_binop("ge", left, right),
        CmpOp::Is => make_binop("is_", left, right),
        CmpOp::IsNot => make_binop("is_not", left, right),
        CmpOp::In => make_binop("contains", right, left),
        CmpOp::NotIn => make_unaryop("not_", make_binop("contains", right, left)),
    }
}

fn assign_to_target(target: &str, value: Expr) -> Vec<Stmt> {
    py_stmt!("{target:id} = {value:expr}", target = target, value = value,)
}

fn target_expr(target: &str) -> Expr {
    py_expr!("\n{target:id}", target = target,)
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_expr_to_stmt.txt");
}
