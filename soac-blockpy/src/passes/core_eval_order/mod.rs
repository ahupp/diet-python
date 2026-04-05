use crate::block_py::{
    instr_any, Await, Block, BlockPyFunction, BlockPyNameLike, BlockTerm,
    CoreBlockPyExprWithAwaitAndYield, CoreBlockPyExprWithYield, Del, HasMeta, Instr, Load,
    MapInstr, MapTerm, Mappable, Store, UnresolvedName, WithMeta, Yield, YieldFrom,
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

fn expr_contains_suspend(expr: &CoreBlockPyExprWithAwaitAndYield) -> bool {
    instr_any(expr, |expr| {
        matches!(
            expr,
            CoreBlockPyExprWithAwaitAndYield::Await(_)
                | CoreBlockPyExprWithAwaitAndYield::Yield(_)
                | CoreBlockPyExprWithAwaitAndYield::YieldFrom(_)
        )
    })
}

fn hoist_core_expr_if_contains_suspend(
    expr: CoreBlockPyExprWithAwaitAndYield,
    out: &mut Vec<CoreBlockPyExprWithAwaitAndYield>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithAwaitAndYield {
    let expr = make_eval_order_explicit_in_core_expr(expr, out, cleanup);
    if expr_contains_suspend(&expr) {
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
    expr: CoreBlockPyExprWithAwaitAndYield,
    out: &mut Vec<CoreBlockPyExprWithAwaitAndYield>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithAwaitAndYield {
    match_default!(expr: crate::passes::CoreBlockPyExprWithAwaitAndYield {
        CoreBlockPyExprWithAwaitAndYield::Literal(_) => expr,
        CoreBlockPyExprWithAwaitAndYield::Await(await_expr) => {
            let meta = await_expr.meta();
            CoreBlockPyExprWithAwaitAndYield::Await(
                Await::new(hoist_core_expr_if_contains_suspend(
                    *await_expr.value,
                    out,
                    cleanup,
                ))
                .with_meta(meta),
            )
        },
        CoreBlockPyExprWithAwaitAndYield::Yield(yield_expr) => {
            let meta = yield_expr.meta();
            CoreBlockPyExprWithAwaitAndYield::Yield(
                Yield::new(hoist_core_expr_if_contains_suspend(
                    *yield_expr.value,
                    out,
                    cleanup,
                ))
                .with_meta(meta),
            )
        },
        CoreBlockPyExprWithAwaitAndYield::YieldFrom(yield_from_expr) => {
            let meta = yield_from_expr.meta();
            CoreBlockPyExprWithAwaitAndYield::YieldFrom(
                YieldFrom::new(hoist_core_expr_if_contains_suspend(
                    *yield_from_expr.value,
                    out,
                    cleanup,
                ))
                .with_meta(meta),
            )
        },
        rest => rest
            .map_same_children(&mut |value| {
            hoist_core_expr_if_contains_suspend(value, out, cleanup)
        })
            .into(),
    })
}

fn append_stmt_cleanup<E>(out: &mut Vec<E>, cleanup: Vec<ast::ExprName>)
where
    E: Instr<Name = UnresolvedName> + From<Del<E>>,
{
    for temp in cleanup.into_iter().rev() {
        out.push(typed_del_expr(temp));
    }
}

struct HoistSuspendsInCoreTerm<'a, 'b> {
    out: &'a mut Vec<CoreBlockPyExprWithAwaitAndYield>,
    cleanup: &'b mut Vec<ast::ExprName>,
}

impl MapInstr<CoreBlockPyExprWithAwaitAndYield, CoreBlockPyExprWithAwaitAndYield>
    for HoistSuspendsInCoreTerm<'_, '_>
{
    fn map_instr(
        &mut self,
        expr: CoreBlockPyExprWithAwaitAndYield,
    ) -> CoreBlockPyExprWithAwaitAndYield {
        hoist_core_expr_if_contains_suspend(expr, self.out, self.cleanup)
    }

    fn map_name(&mut self, name: UnresolvedName) -> UnresolvedName {
        name
    }
}

fn make_eval_order_explicit_in_core_term(
    term: BlockTerm<CoreBlockPyExprWithAwaitAndYield>,
    out: &mut Vec<CoreBlockPyExprWithAwaitAndYield>,
) -> BlockTerm<CoreBlockPyExprWithAwaitAndYield> {
    let mut cleanup = Vec::new();
    let mut map = HoistSuspendsInCoreTerm {
        out,
        cleanup: &mut cleanup,
    };
    map.map_term(term)
}

pub(crate) fn make_eval_order_explicit_in_core_block(
    block: Block<CoreBlockPyExprWithAwaitAndYield>,
) -> Block<CoreBlockPyExprWithAwaitAndYield> {
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
        let expr = make_eval_order_explicit_in_core_expr(expr, &mut setup, &mut cleanup);
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
}

fn is_core_atom_without_await(expr: &CoreBlockPyExprWithYield) -> bool {
    matches!(
        expr,
        CoreBlockPyExprWithYield::Literal(_) | CoreBlockPyExprWithYield::Load(_)
    )
}

fn expr_contains_yield(expr: &CoreBlockPyExprWithYield) -> bool {
    instr_any(expr, |expr| {
        matches!(
            expr,
            CoreBlockPyExprWithYield::Yield(_) | CoreBlockPyExprWithYield::YieldFrom(_)
        )
    })
}

fn hoist_core_expr_without_await_to_atom(
    expr: CoreBlockPyExprWithYield,
    out: &mut Vec<CoreBlockPyExprWithYield>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithYield {
    let expr = make_eval_order_explicit_in_core_expr_without_await(expr, out, cleanup);
    if is_core_atom_without_await(&expr) {
        expr
    } else {
        let target = fresh_eval_name();
        out.push(typed_store_expr(target.clone(), expr));
        cleanup.push(target.clone());
        let meta = target.meta();
        Load::new(target).with_meta(meta).into()
    }
}

fn make_eval_order_explicit_in_core_expr_without_await(
    expr: CoreBlockPyExprWithYield,
    out: &mut Vec<CoreBlockPyExprWithYield>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithYield {
    match_default!(expr: crate::passes::CoreBlockPyExprWithYield {
        CoreBlockPyExprWithYield::Literal(_) => expr,
        CoreBlockPyExprWithYield::Yield(yield_expr) => {
            let meta = yield_expr.meta();
            CoreBlockPyExprWithYield::Yield(
                Yield::new(hoist_core_expr_without_await_to_atom(
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
                YieldFrom::new(hoist_core_expr_without_await_to_atom(
                    *yield_from_expr.value,
                    out,
                    cleanup,
                ))
                .with_meta(meta),
            )
        },
        rest => rest
            .map_same_children(&mut |value| {
            hoist_core_expr_without_await_to_atom(value, out, cleanup)
        })
            .into(),
    })
}

struct HoistYieldFreeAtomsInCoreTerm<'a, 'b> {
    out: &'a mut Vec<CoreBlockPyExprWithYield>,
    cleanup: &'b mut Vec<ast::ExprName>,
}

impl MapInstr<CoreBlockPyExprWithYield, CoreBlockPyExprWithYield>
    for HoistYieldFreeAtomsInCoreTerm<'_, '_>
{
    fn map_instr(&mut self, expr: CoreBlockPyExprWithYield) -> CoreBlockPyExprWithYield {
        hoist_core_expr_without_await_to_atom(expr, self.out, self.cleanup)
    }

    fn map_name(&mut self, name: UnresolvedName) -> UnresolvedName {
        name
    }
}

fn make_eval_order_explicit_in_core_term_without_await(
    term: BlockTerm<CoreBlockPyExprWithYield>,
    out: &mut Vec<CoreBlockPyExprWithYield>,
) -> BlockTerm<CoreBlockPyExprWithYield> {
    let mut cleanup = Vec::new();
    let mut map = HoistYieldFreeAtomsInCoreTerm {
        out,
        cleanup: &mut cleanup,
    };
    map.map_term(term)
}

pub(crate) fn make_eval_order_explicit_in_core_callable_def_without_await(
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
                make_eval_order_explicit_in_core_expr_without_await(
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
        let term = make_eval_order_explicit_in_core_term_without_await(input_term, &mut body);
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
