use crate::template::{make_binop, make_unaryop, single_stmt};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, CmpOp, Expr, Stmt};
pub(crate) fn expr_boolop_to_stmts(target: &str, bool_op: ast::ExprBoolOp) -> Vec<Stmt> {
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

pub(crate) fn expr_compare_to_stmts(target: &str, compare: ast::ExprCompare) -> Vec<Stmt> {
    let ast::ExprCompare {
        left,
        ops,
        comparators,
        ..
    } = compare;

    let mut ops = ops.into_vec().into_iter();
    let mut comparators = comparators.into_vec().into_iter();

    let first_op = ops
        .next()
        .expect("compare expects at least one comparison operator");
    let first_comparator = comparators
        .next()
        .expect("compare expects at least one comparator");

    let mut stmts = vec![assign_to_target(
        target,
        compare_expr(first_op, *left, first_comparator.clone()),
    )];

    let mut current_left = first_comparator;

    for (op, comparator) in ops.zip(comparators) {
        let body_stmt = assign_to_target(
            target,
            compare_expr(op, current_left.clone(), comparator.clone()),
        );
        let stmt = py_stmt!(
            "\nif {test:expr}:\n    {body:stmt}",
            test = target_expr(target),
            body = body_stmt,
        );
        stmts.push(stmt);
        current_left = comparator;
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

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_rewrite_expr_to_stmt.txt");
}
