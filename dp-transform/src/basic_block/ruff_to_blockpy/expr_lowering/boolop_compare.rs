use crate::basic_block::ast_to_ast::ast_rewrite::LoweredExpr;
use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::ast_to_ast::rewrite_expr::{make_binop, make_unaryop};
use crate::template::into_body;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, CmpOp, Expr, Stmt};

pub(super) fn expr_boolop_to_stmts(context: &Context, bool_op: ast::ExprBoolOp) -> LoweredExpr {
    let target = context.fresh("target");

    LoweredExpr::modified(
        py_expr!("{target:id}", target = target.as_str()),
        expr_boolop_to_stmts_inner(target.as_str(), bool_op),
    )
}

fn expr_boolop_to_stmts_inner(target: &str, bool_op: ast::ExprBoolOp) -> Stmt {
    let ast::ExprBoolOp { op, values, .. } = bool_op;

    let mut values = values.into_iter();
    let first = values.next().expect("bool op expects at least one value");
    let stmts = match first {
        Expr::BoolOp(bool_op) => expr_boolop_to_stmts_inner(target, bool_op),
        other => py_stmt!("{target:id} = {value:expr}", target = target, value = other),
    };
    let mut stmts = vec![stmts];

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
        stmts.push(stmt);
    }

    into_body(stmts)
}

pub(super) fn expr_compare_to_stmts(context: &Context, compare: ast::ExprCompare) -> LoweredExpr {
    let ast::ExprCompare {
        left,
        ops,
        comparators,
        ..
    } = compare;

    let ops = ops.into_vec();
    let comparators = comparators.into_vec();
    let count = ops.len();

    if count == 1 {
        return LoweredExpr::modified(
            compare_expr(ops[0], *left.clone(), comparators[0].clone()),
            Stmt::BodyStmt(ast::StmtBody {
                body: Vec::new(),
                range: Default::default(),
                node_index: Default::default(),
            }),
        );
    }

    let mut current_left = *left;
    let target = context.fresh("target");

    let mut steps: Vec<(Vec<Stmt>, Expr)> = Vec::with_capacity(count);
    let mut left_prelude: Vec<Stmt> = Vec::new();
    if count > 1 {
        let left_tmp = context.fresh("compare");
        left_prelude.push(py_stmt!(
            "{tmp:id} = {value:expr}",
            tmp = left_tmp.as_str(),
            value = current_left.clone(),
        ));
        current_left = py_expr!("{tmp:id}", tmp = left_tmp.as_str());
    }

    for (index, (op, comparator)) in ops.into_iter().zip(comparators.into_iter()).enumerate() {
        let mut comparator_expr = comparator;
        let mut prelude = Vec::new();
        if index == 0 {
            prelude.extend(left_prelude.clone());
        }
        if index < count - 1 {
            let tmp = context.fresh("compare");
            prelude.push(py_stmt!(
                "{tmp:id} = {value:expr}",
                tmp = tmp.as_str(),
                value = comparator_expr.clone(),
            ));
            comparator_expr = py_expr!("{tmp:id}", tmp = tmp.as_str());
        }

        let comparison = compare_expr(op, current_left.clone(), comparator_expr.clone());
        steps.push((prelude, comparison));
        current_left = comparator_expr;
    }

    let mut stmt = Stmt::BodyStmt(ast::StmtBody {
        body: Vec::new(),
        range: Default::default(),
        node_index: Default::default(),
    });
    for (prelude, comparison) in steps.into_iter().rev() {
        if matches!(&stmt, Stmt::BodyStmt(ast::StmtBody { body, .. }) if body.is_empty()) {
            let mut stmts = prelude;
            stmts.push(py_stmt!(
                "{target:id} = {value:expr}",
                target = target.as_str(),
                value = comparison
            ));
            stmt = into_body(stmts);
        } else {
            stmt = py_stmt!(
                r#"
{prelude:stmt}
{target:id} = {value:expr}
if {target:id}:
    {body:stmt}
"#,
                prelude = prelude,
                target = target.as_str(),
                value = comparison,
                body = stmt,
            );
        }
    }

    LoweredExpr::modified(py_expr!("{tmp:id}", tmp = target.as_str()), stmt)
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

#[cfg(test)]
mod tests {
    use super::expr_boolop_to_stmts;
    use crate::basic_block::ast_to_ast::{context::Context, Options};
    use crate::py_expr;

    #[test]
    fn expr_head_simplify_rewrites_boolop_for_blockpy() {
        let context = Context::new(Options::for_test(), "");
        let lowered = expr_boolop_to_stmts(
            &context,
            match py_expr!("a and b") {
                ruff_python_ast::Expr::BoolOp(bool_op) => bool_op,
                other => panic!("expected boolop, got {other:?}"),
            },
        );
        let rendered = crate::ruff_ast_to_string(&lowered.stmt);
        assert!(rendered.contains("if _dp_target"), "{rendered}");
    }
}
