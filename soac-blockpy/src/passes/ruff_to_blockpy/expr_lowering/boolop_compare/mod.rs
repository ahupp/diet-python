use super::{BlockPySetupExprLowerer, RuffToBlockPyExpr};
use crate::block_py::{
    BlockPyAssign, BlockPyCfgFragment, BlockPyIf, BlockPyStmtFragmentBuilder, BlockPyTerm, Instr,
    InstrName, StructuredBlockPyStmtFor,
};
use crate::passes::ruff_to_blockpy::expr_lowering::fresh_setup_name;
use crate::passes::ruff_to_blockpy::LoopContext;
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

fn assign_name<E>(target: &str, value: Expr) -> StructuredBlockPyStmtFor<E>
where
    E: From<Expr> + Instr,
    InstrName<E>: From<ast::ExprName>,
{
    StructuredBlockPyStmtFor::Assign(BlockPyAssign {
        target: store_name(target).into(),
        value: value.into(),
    })
}

fn empty_fragment<E>() -> BlockPyCfgFragment<StructuredBlockPyStmtFor<E>, BlockPyTerm<E>>
where
    E: std::fmt::Debug + Instr,
{
    BlockPyCfgFragment::from_stmts(Vec::new())
}

pub(super) fn lower_boolop_into<L, E>(
    lowerer: &L,
    bool_op: ast::ExprBoolOp,
    out: &mut BlockPyStmtFragmentBuilder<E>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<Expr, String>
where
    L: BlockPySetupExprLowerer + ?Sized,
    E: RuffToBlockPyExpr,
{
    let ast::ExprBoolOp { op, values, .. } = bool_op;
    let target = fresh_setup_name("target");
    let mut values = values.into_iter();
    let first = values.next().expect("bool op expects at least one value");
    let first = lowerer.lower_expr_ast_into(first, out, loop_ctx, next_label_id)?;
    out.push_stmt(assign_name(&target, first));

    for value in values {
        let mut body = BlockPyStmtFragmentBuilder::<E>::new();
        let value = lowerer.lower_expr_ast_into(value, &mut body, loop_ctx, next_label_id)?;
        body.push_stmt(assign_name(&target, value));
        let test = match op {
            ast::BoolOp::And => load_name(&target),
            ast::BoolOp::Or => py_expr!("not {target:id}", target = target.as_str()),
        };
        out.push_stmt(StructuredBlockPyStmtFor::If(BlockPyIf {
            test: test.into(),
            body: body.finish(),
            orelse: empty_fragment(),
        }));
    }

    Ok(load_name(&target))
}

pub(super) fn lower_compare_into<L, E>(
    lowerer: &L,
    compare: ast::ExprCompare,
    out: &mut BlockPyStmtFragmentBuilder<E>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<Expr, String>
where
    L: BlockPySetupExprLowerer + ?Sized,
    E: RuffToBlockPyExpr,
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
        let left = lowerer.lower_expr_ast_into(*left, out, loop_ctx, next_label_id)?;
        let right = lowerer.lower_expr_ast_into(
            comparators.into_iter().next().expect("single comparator"),
            out,
            loop_ctx,
            next_label_id,
        )?;
        return Ok(compare_expr(ops[0], left, right));
    }

    let compare_name = fresh_setup_name("compare");
    let mut current_left = lowerer.lower_expr_ast_into(*left, out, loop_ctx, next_label_id)?;
    out.push_stmt(assign_name(&compare_name, current_left));
    current_left = load_name(&compare_name);

    let target_name = fresh_setup_name("target");
    let mut steps = ops.into_iter().zip(comparators.into_iter()).peekable();
    let Some((first_op, first_comparator)) = steps.next() else {
        unreachable!("compare chain should contain at least one step");
    };
    let mut first_comparator =
        lowerer.lower_expr_ast_into(first_comparator, out, loop_ctx, next_label_id)?;
    if steps.peek().is_some() {
        let tmp_name = fresh_setup_name("compare");
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
        let mut comparator_expr =
            lowerer.lower_expr_ast_into(comparator, &mut step_body, loop_ctx, next_label_id)?;
        if steps.peek().is_some() {
            let tmp_name = fresh_setup_name("compare");
            step_body.push_stmt(assign_name(&tmp_name, comparator_expr));
            comparator_expr = load_name(&tmp_name);
        }
        step_body.push_stmt(assign_name(
            &target_name,
            compare_expr(op, current_left.clone(), comparator_expr.clone()),
        ));
        current_left = comparator_expr;
        out.push_stmt(StructuredBlockPyStmtFor::If(BlockPyIf {
            test: load_name(&target_name).into(),
            body: step_body.finish(),
            orelse: empty_fragment(),
        }));
    }

    Ok(load_name(&target_name))
}

fn compare_expr(op: CmpOp, left: Expr, right: Expr) -> Expr {
    Expr::Compare(ast::ExprCompare {
        left: Box::new(left),
        ops: vec![op].into(),
        comparators: vec![right].into(),
        range: Default::default(),
        node_index: Default::default(),
    })
}

#[cfg(test)]
mod test;
