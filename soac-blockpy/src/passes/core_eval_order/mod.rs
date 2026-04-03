use crate::block_py::{
    expr_any, Await, Block, BlockBuilder, BlockPyFunction, BlockPyNameLike, BlockTerm,
    CoreBlockPyExprWithAwaitAndYield, CoreBlockPyExprWithYield, Del, HasMeta, Instr, Load, Store,
    StructuredIf, StructuredInstr, TermBranchTable, TermIf, TermRaise, UnresolvedName, Walkable,
    WithMeta, Yield, YieldFrom,
};
use crate::namegen::fresh_name;
use crate::passes::ruff_to_blockpy::lower_structured_blocks_to_bb_blocks;
use crate::passes::CoreBlockPyPassWithYield;
use crate::py_expr;
use ruff_python_ast as ast;

fn fresh_eval_name() -> ast::ExprName {
    let name = fresh_name("eval");
    let ast::Expr::Name(expr) = py_expr!("{name:id}", name = name.as_str()) else {
        unreachable!();
    };
    expr
}

fn typed_store_stmt<E, N>(target: N, value: E) -> StructuredInstr<E>
where
    E: Instr + From<Store<E>>,
    N: BlockPyNameLike + HasMeta + Into<<E as Instr>::Name>,
{
    let meta = target.meta();
    StructuredInstr::Expr(Store::<E>::new(target, value).with_meta(meta).into())
}

fn typed_del_stmt<E, N>(target: N) -> StructuredInstr<E>
where
    E: Instr<Name = UnresolvedName> + From<Del<E>>,
    N: HasMeta + Into<<E as Instr>::Name>,
{
    let meta = target.meta();
    let target = target.into();
    StructuredInstr::Expr(Del::<E>::new(target, false).with_meta(meta).into())
}

fn expr_contains_suspend(expr: &CoreBlockPyExprWithAwaitAndYield) -> bool {
    expr_any(expr, |expr| {
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
    out: &mut Vec<StructuredInstr<CoreBlockPyExprWithAwaitAndYield>>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithAwaitAndYield {
    let expr = make_eval_order_explicit_in_core_expr(expr, out, cleanup);
    if expr_contains_suspend(&expr) {
        let target = fresh_eval_name();
        out.push(typed_store_stmt(target.clone(), expr));
        cleanup.push(target.clone());
        let meta = target.meta();
        Load::new(target).with_meta(meta).into()
    } else {
        expr
    }
}

fn make_eval_order_explicit_in_core_expr(
    expr: CoreBlockPyExprWithAwaitAndYield,
    out: &mut Vec<StructuredInstr<CoreBlockPyExprWithAwaitAndYield>>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithAwaitAndYield {
    match expr {
        CoreBlockPyExprWithAwaitAndYield::Literal(_) => expr,
        CoreBlockPyExprWithAwaitAndYield::BinOp(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::UnaryOp(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::Call(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::GetAttr(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::SetAttr(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::GetItem(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::SetItem(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::DelItem(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::Load(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::Store(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::Del(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::MakeCell(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::CellRefForName(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::CellRef(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::MakeFunction(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
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
        }
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
        }
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
        }
    }
}

fn append_stmt_cleanup<E>(out: &mut Vec<StructuredInstr<E>>, cleanup: Vec<ast::ExprName>)
where
    E: Instr<Name = UnresolvedName> + From<Del<E>>,
{
    for temp in cleanup.into_iter().rev() {
        out.push(typed_del_stmt(temp));
    }
}

fn make_eval_order_explicit_in_core_fragment(
    fragment: BlockBuilder<
        StructuredInstr<CoreBlockPyExprWithAwaitAndYield>,
        BlockTerm<CoreBlockPyExprWithAwaitAndYield>,
    >,
) -> BlockBuilder<
    StructuredInstr<CoreBlockPyExprWithAwaitAndYield>,
    BlockTerm<CoreBlockPyExprWithAwaitAndYield>,
> {
    let mut body = Vec::new();
    for stmt in fragment.body {
        make_eval_order_explicit_in_core_stmt(stmt, &mut body);
    }
    let term = fragment
        .term
        .map(|term| make_eval_order_explicit_in_core_term(term, &mut body));
    BlockBuilder { body, term }
}

fn make_eval_order_explicit_in_core_stmt(
    stmt: StructuredInstr<CoreBlockPyExprWithAwaitAndYield>,
    out: &mut Vec<StructuredInstr<CoreBlockPyExprWithAwaitAndYield>>,
) {
    match stmt {
        StructuredInstr::Expr(expr) => {
            let mut setup = Vec::new();
            let mut cleanup = Vec::new();
            let expr = make_eval_order_explicit_in_core_expr(expr, &mut setup, &mut cleanup);
            out.extend(setup);
            out.push(StructuredInstr::Expr(expr));
            append_stmt_cleanup(out, cleanup);
        }
        StructuredInstr::If(if_stmt) => {
            let mut setup = Vec::new();
            let mut cleanup = Vec::new();
            let test = hoist_core_expr_if_contains_suspend(if_stmt.test, &mut setup, &mut cleanup);
            out.extend(setup);
            out.push(StructuredInstr::If(StructuredIf {
                test,
                body: make_eval_order_explicit_in_core_fragment(if_stmt.body),
                orelse: make_eval_order_explicit_in_core_fragment(if_stmt.orelse),
            }));
            append_stmt_cleanup(out, cleanup);
        }
    }
}

fn make_eval_order_explicit_in_core_term(
    term: BlockTerm<CoreBlockPyExprWithAwaitAndYield>,
    out: &mut Vec<StructuredInstr<CoreBlockPyExprWithAwaitAndYield>>,
) -> BlockTerm<CoreBlockPyExprWithAwaitAndYield> {
    match term {
        BlockTerm::Jump(edge) => BlockTerm::Jump(edge),
        BlockTerm::IfTerm(TermIf {
            test,
            then_label,
            else_label,
        }) => BlockTerm::IfTerm(TermIf {
            test: hoist_core_expr_if_contains_suspend(test, out, &mut Vec::new()),
            then_label,
            else_label,
        }),
        BlockTerm::BranchTable(TermBranchTable {
            index,
            targets,
            default_label,
        }) => BlockTerm::BranchTable(TermBranchTable {
            index: hoist_core_expr_if_contains_suspend(index, out, &mut Vec::new()),
            targets,
            default_label,
        }),
        BlockTerm::Raise(TermRaise { exc }) => BlockTerm::Raise(TermRaise {
            exc: exc.map(|value| hoist_core_expr_if_contains_suspend(value, out, &mut Vec::new())),
        }),
        BlockTerm::Return(value) => BlockTerm::Return(hoist_core_expr_if_contains_suspend(
            value,
            out,
            &mut Vec::new(),
        )),
    }
}

pub(crate) fn make_eval_order_explicit_in_core_block(
    block: Block<
        StructuredInstr<CoreBlockPyExprWithAwaitAndYield>,
        CoreBlockPyExprWithAwaitAndYield,
    >,
) -> Block<StructuredInstr<CoreBlockPyExprWithAwaitAndYield>, CoreBlockPyExprWithAwaitAndYield> {
    let Block {
        label,
        body: input_body,
        term: input_term,
        params,
        exc_edge,
    } = block;
    let mut body = Vec::new();
    for stmt in input_body {
        make_eval_order_explicit_in_core_stmt(stmt, &mut body);
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
    expr_any(expr, |expr| {
        matches!(
            expr,
            CoreBlockPyExprWithYield::Yield(_) | CoreBlockPyExprWithYield::YieldFrom(_)
        )
    })
}

fn hoist_core_expr_without_await_to_atom(
    expr: CoreBlockPyExprWithYield,
    out: &mut Vec<StructuredInstr<CoreBlockPyExprWithYield>>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithYield {
    let expr = make_eval_order_explicit_in_core_expr_without_await(expr, out, cleanup);
    if is_core_atom_without_await(&expr) {
        expr
    } else {
        let target = fresh_eval_name();
        out.push(typed_store_stmt(target.clone(), expr));
        cleanup.push(target.clone());
        let meta = target.meta();
        Load::new(target).with_meta(meta).into()
    }
}

fn make_eval_order_explicit_in_core_expr_without_await(
    expr: CoreBlockPyExprWithYield,
    out: &mut Vec<StructuredInstr<CoreBlockPyExprWithYield>>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithYield {
    match expr {
        CoreBlockPyExprWithYield::Literal(_) => expr,
        CoreBlockPyExprWithYield::BinOp(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::UnaryOp(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::Call(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::GetAttr(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::SetAttr(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::GetItem(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::SetItem(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::DelItem(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::Load(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::Store(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::Del(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::MakeCell(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::CellRefForName(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::CellRef(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::MakeFunction(operation) => operation
            .map_walk(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
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
        }
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
        }
    }
}

fn make_eval_order_explicit_in_core_stmt_without_await(
    stmt: StructuredInstr<CoreBlockPyExprWithYield>,
    out: &mut Vec<StructuredInstr<CoreBlockPyExprWithYield>>,
) {
    match stmt {
        StructuredInstr::Expr(expr) => {
            let mut setup = Vec::new();
            let mut cleanup = Vec::new();
            let expr = if expr_contains_yield(&expr) {
                make_eval_order_explicit_in_core_expr_without_await(expr, &mut setup, &mut cleanup)
            } else {
                expr
            };
            out.extend(setup);
            out.push(StructuredInstr::Expr(expr));
            append_stmt_cleanup(out, cleanup);
        }
        StructuredInstr::If(if_stmt) => {
            let mut setup = Vec::new();
            let mut cleanup = Vec::new();
            let test =
                hoist_core_expr_without_await_to_atom(if_stmt.test, &mut setup, &mut cleanup);
            out.extend(setup);
            out.push(StructuredInstr::If(StructuredIf {
                test,
                body: make_eval_order_explicit_in_core_fragment_without_await(if_stmt.body),
                orelse: make_eval_order_explicit_in_core_fragment_without_await(if_stmt.orelse),
            }));
            append_stmt_cleanup(out, cleanup);
        }
    }
}

fn make_eval_order_explicit_in_core_fragment_without_await(
    fragment: BlockBuilder<
        StructuredInstr<CoreBlockPyExprWithYield>,
        BlockTerm<CoreBlockPyExprWithYield>,
    >,
) -> BlockBuilder<StructuredInstr<CoreBlockPyExprWithYield>, BlockTerm<CoreBlockPyExprWithYield>> {
    let mut body = Vec::new();
    for stmt in fragment.body {
        make_eval_order_explicit_in_core_stmt_without_await(stmt, &mut body);
    }
    let term = fragment
        .term
        .map(|term| make_eval_order_explicit_in_core_term_without_await(term, &mut body));
    BlockBuilder { body, term }
}

fn make_eval_order_explicit_in_core_term_without_await(
    term: BlockTerm<CoreBlockPyExprWithYield>,
    out: &mut Vec<StructuredInstr<CoreBlockPyExprWithYield>>,
) -> BlockTerm<CoreBlockPyExprWithYield> {
    match term {
        BlockTerm::Jump(_) => term,
        BlockTerm::IfTerm(TermIf {
            test,
            then_label,
            else_label,
        }) => BlockTerm::IfTerm(TermIf {
            test: hoist_core_expr_without_await_to_atom(test, out, &mut Vec::new()),
            then_label,
            else_label,
        }),
        BlockTerm::BranchTable(TermBranchTable {
            index,
            targets,
            default_label,
        }) => BlockTerm::BranchTable(TermBranchTable {
            index: hoist_core_expr_without_await_to_atom(index, out, &mut Vec::new()),
            targets,
            default_label,
        }),
        BlockTerm::Raise(TermRaise { exc }) => BlockTerm::Raise(TermRaise {
            exc: exc
                .map(|value| hoist_core_expr_without_await_to_atom(value, out, &mut Vec::new())),
        }),
        BlockTerm::Return(value) => BlockTerm::Return(hoist_core_expr_without_await_to_atom(
            value,
            out,
            &mut Vec::new(),
        )),
    }
}

pub(crate) fn make_eval_order_explicit_in_core_block_without_await(
    block: Block<StructuredInstr<CoreBlockPyExprWithYield>, CoreBlockPyExprWithYield>,
) -> Block<StructuredInstr<CoreBlockPyExprWithYield>, CoreBlockPyExprWithYield> {
    let Block {
        label,
        body: input_body,
        term: input_term,
        params,
        exc_edge,
    } = block;
    let mut body = Vec::new();
    for stmt in input_body {
        make_eval_order_explicit_in_core_stmt_without_await(stmt, &mut body);
    }
    let term = make_eval_order_explicit_in_core_term_without_await(input_term, &mut body);
    Block {
        label,
        body,
        term,
        params,
        exc_edge,
    }
}

pub(crate) fn make_eval_order_explicit_in_core_callable_def_without_await(
    callable_def: BlockPyFunction<CoreBlockPyPassWithYield>,
) -> BlockPyFunction<CoreBlockPyPassWithYield> {
    let BlockPyFunction {
        function_id,
        name_gen,
        names,
        kind,
        params,
        blocks,
        doc,
        storage_layout,
        semantic,
    } = callable_def;
    let structured_blocks = blocks
        .into_iter()
        .map(|block| Block {
            label: block.label,
            body: block.body.into_iter().map(Into::into).collect(),
            term: block.term,
            params: block.params,
            exc_edge: block.exc_edge,
        })
        .map(make_eval_order_explicit_in_core_block_without_await)
        .collect::<Vec<_>>();
    let blocks = lower_structured_blocks_to_bb_blocks(&name_gen, &structured_blocks);
    BlockPyFunction {
        function_id,
        name_gen,
        names,
        kind,
        params,
        blocks,
        doc,
        storage_layout,
        semantic,
    }
}

#[cfg(test)]
mod test;
