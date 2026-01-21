use crate::transform::rewrite_expr::make_binop;
use crate::transform::driver::{ExprRewriter, LoweredExpr};
use crate::transform::rewrite_expr::make_unaryop;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, CmpOp, Expr, Stmt};


pub(crate) fn expr_boolop_to_stmts(rewriter: &ExprRewriter, bool_op: ast::ExprBoolOp) -> LoweredExpr {
    let target = rewriter.context().fresh("target");

    LoweredExpr::modified(
        py_expr!("{target:id}", target=target.as_str()),
        expr_boolop_to_stmts_inner(target.as_str(), bool_op),
    )
}

fn expr_boolop_to_stmts_inner(target: &str, bool_op: ast::ExprBoolOp) -> Vec<Stmt> {
   
    let ast::ExprBoolOp { op, values, .. } = bool_op;

    let mut values = values.into_iter();
    let first = values.next().expect("bool op expects at least one value");
    let mut stmts = match first {
        Expr::BoolOp(bool_op) => expr_boolop_to_stmts_inner(target, bool_op),
        other => py_stmt!("{target:id} = {value:expr}", target = target, value = other),
    };

    for value in values {
        let body_stmt = match value {
            Expr::BoolOp(bool_op) => expr_boolop_to_stmts_inner(target, bool_op),
            other => py_stmt!("{target:id} = {value:expr}", target = target, value = other),
        };
        let test_expr = match op {
            ast::BoolOp::And => py_expr!("{target:id}", target = target),
            ast::BoolOp::Or => py_expr!("not {target:id}", target = target),
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
    };

    stmts
}

pub(crate) fn expr_compare_to_stmts(
    rewriter: &ExprRewriter,
    compare: ast::ExprCompare,
) -> LoweredExpr {
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

    let target = rewriter.context().fresh("target");

    for (index, (op, comparator)) in ops.into_iter().zip(comparators.into_iter()).enumerate() {
        let mut comparator_expr = comparator;
        let mut prelude = Vec::new();
        if index < count - 1 {
            let tmp = rewriter.context().fresh("compare");
            prelude.extend(py_stmt!(
                "{tmp:id} = {value:expr}",
                tmp = tmp.as_str(),
                value = comparator_expr.clone(),
            ));
            comparator_expr = py_expr!("{tmp:id}", tmp = tmp.as_str());
        }

        let comparison = compare_expr(op, current_left.clone(), comparator_expr.clone());

        if index == 0 {
            stmts.extend(py_stmt!(
                "{target:id} = {value:expr}",
                target = target.as_str(),
                value = comparison,
            ));
        } else {
            stmts = py_stmt!(
                r#"
{prelude:stmt}
{target:id} = {value:expr}                
if {target:id}:
    {body:stmt}
"#,
                prelude = prelude,
                target = target.as_str(),
                value = comparison,
                body = stmts,
            );
        }

        current_left = comparator_expr;
    }

    LoweredExpr::modified(
        py_expr!("{tmp:id}", tmp = target.as_str()),
        stmts,
    )
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
