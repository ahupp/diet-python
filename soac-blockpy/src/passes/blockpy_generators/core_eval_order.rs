use crate::block_py::{
    instr_any, Block, BlockPyFunction, BlockPyNameLike, BlockTerm, CoreBlockPyExprWithYield,
    Del, HasMeta, Instr, Load, MapInstr, MapTerm, Mappable, Store, UnresolvedName, WithMeta,
    Yield, YieldFrom,
};
use crate::namegen::fresh_name;
use crate::passes::CoreBlockPyPassWithYield;
use crate::py_expr;
use ruff_python_ast as ast;
use soac_macros::match_default;

fn fresh_eval_name() -> ast::ExprName {
    let name = fresh_name("eval");
    let ast::Expr::Name(expr) = py_expr!("{name:id}", name = name.as_str()) else {
        unreachable!();
    };
    expr
}

fn typed_store_expr<E, N>(target: N, value: E) -> E
where
    E: Instr + From<Store<E>>,
    N: BlockPyNameLike + HasMeta + Into<<E as Instr>::Name>,
{
    let meta = target.meta();
    Store::<E>::new(target, value).with_meta(meta).into()
}

fn typed_del_expr<E, N>(target: N) -> E
where
    E: Instr<Name = UnresolvedName> + From<Del<E>>,
    N: HasMeta + Into<<E as Instr>::Name>,
{
    let meta = target.meta();
    let target = target.into();
    Del::<E>::new(target, false).with_meta(meta).into()
}

fn append_stmt_cleanup<E>(out: &mut Vec<E>, cleanup: Vec<ast::ExprName>)
where
    E: Instr<Name = UnresolvedName> + From<Del<E>>,
{
    for temp in cleanup.into_iter().rev() {
        out.push(typed_del_expr(temp));
    }
}

fn expr_contains_yield(expr: &CoreBlockPyExprWithYield) -> bool {
    instr_any(expr, |expr| {
        matches!(
            expr,
            CoreBlockPyExprWithYield::Yield(_) | CoreBlockPyExprWithYield::YieldFrom(_)
        )
    })
}

fn hoist_core_expr_if_contains_yield(
    expr: CoreBlockPyExprWithYield,
    out: &mut Vec<CoreBlockPyExprWithYield>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithYield {
    let expr = make_eval_order_explicit_in_core_expr(expr, out, cleanup);
    if expr_contains_yield(&expr) {
        let target = fresh_eval_name();
        out.push(typed_store_expr(target.clone(), expr));
        cleanup.push(target.clone());
        let meta = target.meta();
        Load::new(target).with_meta(meta).into()
    } else {
        expr
    }
}

fn make_eval_order_explicit_in_core_expr(
    expr: CoreBlockPyExprWithYield,
    out: &mut Vec<CoreBlockPyExprWithYield>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithYield {
    match_default!(expr: crate::passes::CoreBlockPyExprWithYield {
        CoreBlockPyExprWithYield::Literal(_) => expr,
        CoreBlockPyExprWithYield::Yield(yield_expr) => {
            let meta = yield_expr.meta();
            CoreBlockPyExprWithYield::Yield(
                Yield::new(hoist_core_expr_if_contains_yield(
                    *yield_expr.value,
                    out,
                    cleanup,
                ))
                .with_meta(meta),
            )
        },
        CoreBlockPyExprWithYield::YieldFrom(yield_from_expr) => {
            let meta = yield_from_expr.meta();
            CoreBlockPyExprWithYield::YieldFrom(
                YieldFrom::new(hoist_core_expr_if_contains_yield(
                    *yield_from_expr.value,
                    out,
                    cleanup,
                ))
                .with_meta(meta),
            )
        },
        rest => rest
            .map_same_children(&mut |value| {
            hoist_core_expr_if_contains_yield(value, out, cleanup)
        })
            .into(),
    })
}

struct HoistYieldAtomsInCoreTerm<'a, 'b> {
    out: &'a mut Vec<CoreBlockPyExprWithYield>,
    cleanup: &'b mut Vec<ast::ExprName>,
}

impl MapInstr<CoreBlockPyExprWithYield, CoreBlockPyExprWithYield>
    for HoistYieldAtomsInCoreTerm<'_, '_>
{
    fn map_instr(&mut self, expr: CoreBlockPyExprWithYield) -> CoreBlockPyExprWithYield {
        hoist_core_expr_if_contains_yield(expr, self.out, self.cleanup)
    }

    fn map_name(&mut self, name: UnresolvedName) -> UnresolvedName {
        name
    }
}

fn make_eval_order_explicit_in_core_term(
    term: BlockTerm<CoreBlockPyExprWithYield>,
    out: &mut Vec<CoreBlockPyExprWithYield>,
) -> BlockTerm<CoreBlockPyExprWithYield> {
    let mut cleanup = Vec::new();
    let mut map = HoistYieldAtomsInCoreTerm {
        out,
        cleanup: &mut cleanup,
    };
    map.map_term(term)
}

pub(crate) fn make_eval_order_explicit_in_core_callable_def(
    callable_def: BlockPyFunction<CoreBlockPyPassWithYield>,
) -> BlockPyFunction<CoreBlockPyPassWithYield> {
    callable_def.map_blocks(|block| {
        let Block {
            label,
            body: input_body,
            term: input_term,
            params,
            exc_edge,
        } = block;
        let mut body = Vec::new();
        for expr in input_body {
            let mut setup = Vec::new();
            let mut cleanup = Vec::new();
            let expr = if expr_contains_yield(&expr) {
                make_eval_order_explicit_in_core_expr(
                    expr,
                    &mut setup,
                    &mut cleanup,
                )
            } else {
                expr
            };
            body.extend(setup);
            body.push(expr);
            append_stmt_cleanup(&mut body, cleanup);
        }
        let term = make_eval_order_explicit_in_core_term(input_term, &mut body);
        Block {
            label,
            body,
            term,
            params,
            exc_edge,
        }
    })
}

#[cfg(test)]
mod test;
