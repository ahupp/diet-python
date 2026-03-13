use super::BlockPySetupExprLowerer;
use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::ast_to_ast::rewrite_expr::{make_binop, make_unaryop};
use crate::basic_block::block_py::{
    BlockPyAssign, BlockPyCfgFragment, BlockPyIf, BlockPyStmt, BlockPyStmtFragmentBuilder,
    BlockPyTerm,
};
use crate::basic_block::ruff_to_blockpy::LoopContext;
use crate::py_expr;
use ruff_python_ast::{self as ast, CmpOp, Expr};

fn store_name(name: &str) -> ast::ExprName {
    ast::ExprName {
        id: name.into(),
        ctx: ast::ExprContext::Store,
        range: Default::default(),
        node_index: ast::AtomicNodeIndex::default(),
    }
}

fn load_name(name: &str) -> Expr {
    py_expr!("{name:id}", name = name)
}

fn assign_name<E>(target: &str, value: Expr) -> BlockPyStmt<E>
where
    E: From<Expr>,
{
    BlockPyStmt::Assign(BlockPyAssign {
        target: store_name(target),
        value: value.into(),
    })
}

fn empty_fragment<E>() -> BlockPyCfgFragment<BlockPyStmt<E>, BlockPyTerm<E>>
where
    E: std::fmt::Debug,
{
    BlockPyCfgFragment::from_stmts(Vec::new())
}

pub(super) fn lower_boolop_into<L, E>(
    lowerer: &L,
    context: &Context,
    bool_op: ast::ExprBoolOp,
    out: &mut BlockPyStmtFragmentBuilder<E>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<Expr, String>
where
    L: BlockPySetupExprLowerer + ?Sized,
    E: From<Expr> + std::fmt::Debug,
{
    let ast::ExprBoolOp { op, values, .. } = bool_op;
    let target = context.fresh("target");
    let mut values = values.into_iter();
    let first = values.next().expect("bool op expects at least one value");
    let first = lowerer.lower_expr_ast_into(context, first, out, loop_ctx, next_label_id)?;
    out.push_stmt(assign_name(&target, first));

    for value in values {
        let mut body = BlockPyStmtFragmentBuilder::<E>::new();
        let value =
            lowerer.lower_expr_ast_into(context, value, &mut body, loop_ctx, next_label_id)?;
        body.push_stmt(assign_name(&target, value));
        let test = match op {
            ast::BoolOp::And => load_name(&target),
            ast::BoolOp::Or => py_expr!("not {target:id}", target = target.as_str()),
        };
        out.push_stmt(BlockPyStmt::If(BlockPyIf {
            test: test.into(),
            body: body.finish(),
            orelse: empty_fragment(),
        }));
    }

    Ok(load_name(&target))
}

pub(super) fn lower_compare_into<L, E>(
    lowerer: &L,
    context: &Context,
    compare: ast::ExprCompare,
    out: &mut BlockPyStmtFragmentBuilder<E>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<Expr, String>
where
    L: BlockPySetupExprLowerer + ?Sized,
    E: From<Expr> + std::fmt::Debug,
{
    let ast::ExprCompare {
        left,
        ops,
        comparators,
        ..
    } = compare;

    let ops = ops.into_vec();
    let comparators = comparators.into_vec();
    if ops.len() == 1 {
        let left = lowerer.lower_expr_ast_into(context, *left, out, loop_ctx, next_label_id)?;
        let right = lowerer.lower_expr_ast_into(
            context,
            comparators.into_iter().next().expect("single comparator"),
            out,
            loop_ctx,
            next_label_id,
        )?;
        return Ok(compare_expr(ops[0], left, right));
    }

    let compare_name = context.fresh("compare");
    let mut current_left =
        lowerer.lower_expr_ast_into(context, *left, out, loop_ctx, next_label_id)?;
    out.push_stmt(assign_name(&compare_name, current_left));
    current_left = load_name(&compare_name);

    let target_name = context.fresh("target");
    let mut steps = ops.into_iter().zip(comparators.into_iter()).peekable();
    let Some((first_op, first_comparator)) = steps.next() else {
        unreachable!("compare chain should contain at least one step");
    };
    let mut first_comparator =
        lowerer.lower_expr_ast_into(context, first_comparator, out, loop_ctx, next_label_id)?;
    if steps.peek().is_some() {
        let tmp_name = context.fresh("compare");
        out.push_stmt(assign_name(&tmp_name, first_comparator));
        first_comparator = load_name(&tmp_name);
    }
    out.push_stmt(assign_name(
        &target_name,
        compare_expr(first_op, current_left.clone(), first_comparator.clone()),
    ));
    current_left = first_comparator;

    while let Some((op, comparator)) = steps.next() {
        let mut step_body = BlockPyStmtFragmentBuilder::<E>::new();
        let mut comparator_expr = lowerer.lower_expr_ast_into(
            context,
            comparator,
            &mut step_body,
            loop_ctx,
            next_label_id,
        )?;
        if steps.peek().is_some() {
            let tmp_name = context.fresh("compare");
            step_body.push_stmt(assign_name(&tmp_name, comparator_expr));
            comparator_expr = load_name(&tmp_name);
        }
        step_body.push_stmt(assign_name(
            &target_name,
            compare_expr(op, current_left.clone(), comparator_expr.clone()),
        ));
        current_left = comparator_expr;
        out.push_stmt(BlockPyStmt::If(BlockPyIf {
            test: load_name(&target_name).into(),
            body: step_body.finish(),
            orelse: empty_fragment(),
        }));
    }

    Ok(load_name(&target_name))
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
    use crate::basic_block::ast_to_ast::{context::Context, Options};
    use crate::basic_block::block_py::{BlockPyExpr, BlockPyStmt, BlockPyStmtFragmentBuilder};
    use crate::basic_block::ruff_to_blockpy::expr_lowering::lower_expr_into_with_setup;
    use crate::py_expr;

    #[test]
    fn boolop_lowering_emits_blockpy_setup_directly() {
        let context = Context::new(Options::for_test(), "");
        let mut out = BlockPyStmtFragmentBuilder::<BlockPyExpr>::new();
        let mut next_label_id = 0usize;

        let lowered = lower_expr_into_with_setup(
            &context,
            py_expr!("a and b"),
            &mut out,
            None,
            &mut next_label_id,
        )
        .expect("expr lowering should succeed");

        let fragment = out.finish();
        assert!(matches!(lowered, BlockPyExpr::Name(_)), "{lowered:?}");
        assert!(
            fragment
                .body
                .iter()
                .any(|stmt| matches!(stmt, BlockPyStmt::Assign(_))),
            "{fragment:?}"
        );
        assert!(
            fragment
                .body
                .iter()
                .any(|stmt| matches!(stmt, BlockPyStmt::If(_))),
            "{fragment:?}"
        );
    }
}
