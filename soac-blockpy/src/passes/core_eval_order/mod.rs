use crate::block_py::structured::IntoStructuredBlockPyStmt;
use crate::block_py::{
    expr_any, BlockPyAssign, BlockPyBranchTable, BlockPyCfgFragment, BlockPyFunction, BlockPyIf,
    BlockPyIfTerm, BlockPyNameLike, BlockPyRaise, BlockPyTerm, CfgBlock, CoreBlockPyAwait,
    CoreBlockPyExprWithAwaitAndYield, CoreBlockPyExprWithYield, CoreBlockPyYield,
    CoreBlockPyYieldFrom, Del, HasMeta, Instr, InstrExprNode, Meta, Store,
    StructuredBlockPyStmtFor, WithMeta,
};
use crate::namegen::fresh_name;
use crate::passes::ast_to_ast::scope_helpers::is_internal_symbol;
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

fn typed_store_stmt<E, N>(target: N, value: E) -> StructuredBlockPyStmtFor<E>
where
    E: Instr + From<Store<E>>,
    N: BlockPyNameLike + Into<<E as Instr>::Name>,
{
    let meta = Meta::new(target.node_index(), target.range());
    StructuredBlockPyStmtFor::Expr(Store::<E>::new(target, value).with_meta(meta).into())
}

fn typed_del_stmt<E>(target: impl Into<<E as Instr>::Name>) -> StructuredBlockPyStmtFor<E>
where
    E: Instr + From<Del<E>>,
{
    let target = target.into();
    let meta = Meta::new(target.node_index(), target.range());
    StructuredBlockPyStmtFor::Expr(Del::<E>::new(target, false).with_meta(meta).into())
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
    out: &mut Vec<StructuredBlockPyStmtFor<CoreBlockPyExprWithAwaitAndYield>>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithAwaitAndYield {
    let expr = make_eval_order_explicit_in_core_expr(expr, out, cleanup);
    if expr_contains_suspend(&expr) {
        let target = fresh_eval_name();
        out.push(typed_store_stmt(target.clone(), expr));
        cleanup.push(target.clone());
        CoreBlockPyExprWithAwaitAndYield::Name(target.into())
    } else {
        expr
    }
}

fn make_eval_order_explicit_in_core_expr(
    expr: CoreBlockPyExprWithAwaitAndYield,
    out: &mut Vec<StructuredBlockPyStmtFor<CoreBlockPyExprWithAwaitAndYield>>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithAwaitAndYield {
    match expr {
        CoreBlockPyExprWithAwaitAndYield::Name(_)
        | CoreBlockPyExprWithAwaitAndYield::Literal(_) => expr,
        CoreBlockPyExprWithAwaitAndYield::BinOp(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::UnaryOp(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::Call(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::GetAttr(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::SetAttr(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::GetItem(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::SetItem(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::DelItem(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::Load(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::Store(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::Del(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::MakeCell(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::CellRefForName(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::CellRef(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::MakeFunction(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_if_contains_suspend(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithAwaitAndYield::Await(await_expr) => {
            let meta = await_expr.meta();
            CoreBlockPyExprWithAwaitAndYield::Await(
                CoreBlockPyAwait::new(hoist_core_expr_if_contains_suspend(
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
                CoreBlockPyYield::new(hoist_core_expr_if_contains_suspend(
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
                CoreBlockPyYieldFrom::new(hoist_core_expr_if_contains_suspend(
                    *yield_from_expr.value,
                    out,
                    cleanup,
                ))
                .with_meta(meta),
            )
        }
    }
}

fn append_stmt_cleanup<E>(out: &mut Vec<StructuredBlockPyStmtFor<E>>, cleanup: Vec<ast::ExprName>)
where
    E: Instr + From<Del<E>>,
{
    for temp in cleanup.into_iter().rev() {
        out.push(typed_del_stmt(temp));
    }
}

fn make_eval_order_explicit_in_core_fragment(
    fragment: BlockPyCfgFragment<
        StructuredBlockPyStmtFor<CoreBlockPyExprWithAwaitAndYield>,
        BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
    >,
) -> BlockPyCfgFragment<
    StructuredBlockPyStmtFor<CoreBlockPyExprWithAwaitAndYield>,
    BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
> {
    let mut body = Vec::new();
    for stmt in fragment.body {
        make_eval_order_explicit_in_core_stmt(stmt, &mut body);
    }
    let term = fragment
        .term
        .map(|term| make_eval_order_explicit_in_core_term(term, &mut body));
    BlockPyCfgFragment { body, term }
}

fn make_eval_order_explicit_in_core_stmt(
    stmt: StructuredBlockPyStmtFor<CoreBlockPyExprWithAwaitAndYield>,
    out: &mut Vec<StructuredBlockPyStmtFor<CoreBlockPyExprWithAwaitAndYield>>,
) {
    match stmt {
        StructuredBlockPyStmtFor::Expr(expr) => {
            let mut setup = Vec::new();
            let mut cleanup = Vec::new();
            let expr = make_eval_order_explicit_in_core_expr(expr, &mut setup, &mut cleanup);
            out.extend(setup);
            out.push(StructuredBlockPyStmtFor::Expr(expr));
            append_stmt_cleanup(out, cleanup);
        }
        StructuredBlockPyStmtFor::If(if_stmt) => {
            let mut setup = Vec::new();
            let mut cleanup = Vec::new();
            let test = hoist_core_expr_if_contains_suspend(if_stmt.test, &mut setup, &mut cleanup);
            out.extend(setup);
            out.push(StructuredBlockPyStmtFor::If(BlockPyIf {
                test,
                body: make_eval_order_explicit_in_core_fragment(if_stmt.body),
                orelse: make_eval_order_explicit_in_core_fragment(if_stmt.orelse),
            }));
            append_stmt_cleanup(out, cleanup);
        }
        StructuredBlockPyStmtFor::_Marker(_) => {
            unreachable!("structured stmt marker should not appear")
        }
    }
}

fn make_eval_order_explicit_in_core_term(
    term: BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
    out: &mut Vec<StructuredBlockPyStmtFor<CoreBlockPyExprWithAwaitAndYield>>,
) -> BlockPyTerm<CoreBlockPyExprWithAwaitAndYield> {
    match term {
        BlockPyTerm::Jump(edge) => BlockPyTerm::Jump(edge),
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test,
            then_label,
            else_label,
        }) => BlockPyTerm::IfTerm(BlockPyIfTerm {
            test: hoist_core_expr_if_contains_suspend(test, out, &mut Vec::new()),
            then_label,
            else_label,
        }),
        BlockPyTerm::BranchTable(BlockPyBranchTable {
            index,
            targets,
            default_label,
        }) => BlockPyTerm::BranchTable(BlockPyBranchTable {
            index: hoist_core_expr_if_contains_suspend(index, out, &mut Vec::new()),
            targets,
            default_label,
        }),
        BlockPyTerm::Raise(BlockPyRaise { exc }) => BlockPyTerm::Raise(BlockPyRaise {
            exc: exc.map(|value| hoist_core_expr_if_contains_suspend(value, out, &mut Vec::new())),
        }),
        BlockPyTerm::Return(value) => BlockPyTerm::Return(hoist_core_expr_if_contains_suspend(
            value,
            out,
            &mut Vec::new(),
        )),
    }
}

pub(crate) fn make_eval_order_explicit_in_core_block(
    block: CfgBlock<
        StructuredBlockPyStmtFor<CoreBlockPyExprWithAwaitAndYield>,
        BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
    >,
) -> CfgBlock<
    StructuredBlockPyStmtFor<CoreBlockPyExprWithAwaitAndYield>,
    BlockPyTerm<CoreBlockPyExprWithAwaitAndYield>,
> {
    let CfgBlock {
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
    CfgBlock {
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
        CoreBlockPyExprWithYield::Name(_) | CoreBlockPyExprWithYield::Literal(_)
    ) || matches!(expr, CoreBlockPyExprWithYield::Load(_))
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
    out: &mut Vec<StructuredBlockPyStmtFor<CoreBlockPyExprWithYield>>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithYield {
    let expr = make_eval_order_explicit_in_core_expr_without_await(expr, out, cleanup);
    if is_core_atom_without_await(&expr) {
        expr
    } else {
        let target = fresh_eval_name();
        out.push(typed_store_stmt(target.clone(), expr));
        cleanup.push(target.clone());
        CoreBlockPyExprWithYield::Name(target.into())
    }
}

fn make_eval_order_explicit_in_core_expr_without_await(
    expr: CoreBlockPyExprWithYield,
    out: &mut Vec<StructuredBlockPyStmtFor<CoreBlockPyExprWithYield>>,
    cleanup: &mut Vec<ast::ExprName>,
) -> CoreBlockPyExprWithYield {
    match expr {
        CoreBlockPyExprWithYield::Name(_) | CoreBlockPyExprWithYield::Literal(_) => expr,
        CoreBlockPyExprWithYield::BinOp(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::UnaryOp(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::Call(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::GetAttr(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::SetAttr(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::GetItem(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::SetItem(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::DelItem(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::Load(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::Store(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::Del(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::MakeCell(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::CellRefForName(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::CellRef(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::MakeFunction(operation) => operation
            .map_expr_node(&mut |value| hoist_core_expr_without_await_to_atom(value, out, cleanup))
            .into(),
        CoreBlockPyExprWithYield::Yield(yield_expr) => {
            let meta = yield_expr.meta();
            CoreBlockPyExprWithYield::Yield(
                CoreBlockPyYield::new(hoist_core_expr_without_await_to_atom(
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
                CoreBlockPyYieldFrom::new(hoist_core_expr_without_await_to_atom(
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
    stmt: StructuredBlockPyStmtFor<CoreBlockPyExprWithYield>,
    out: &mut Vec<StructuredBlockPyStmtFor<CoreBlockPyExprWithYield>>,
) {
    match stmt {
        StructuredBlockPyStmtFor::Expr(expr) => {
            let mut setup = Vec::new();
            let mut cleanup = Vec::new();
            let expr = if expr_contains_yield(&expr) {
                make_eval_order_explicit_in_core_expr_without_await(expr, &mut setup, &mut cleanup)
            } else {
                expr
            };
            out.extend(setup);
            out.push(StructuredBlockPyStmtFor::Expr(expr));
            append_stmt_cleanup(out, cleanup);
        }
        StructuredBlockPyStmtFor::If(if_stmt) => {
            let mut setup = Vec::new();
            let mut cleanup = Vec::new();
            let test =
                hoist_core_expr_without_await_to_atom(if_stmt.test, &mut setup, &mut cleanup);
            out.extend(setup);
            out.push(StructuredBlockPyStmtFor::If(BlockPyIf {
                test,
                body: make_eval_order_explicit_in_core_fragment_without_await(if_stmt.body),
                orelse: make_eval_order_explicit_in_core_fragment_without_await(if_stmt.orelse),
            }));
            append_stmt_cleanup(out, cleanup);
        }
        StructuredBlockPyStmtFor::_Marker(_) => {
            unreachable!("structured stmt marker should not appear")
        }
    }
}

fn make_eval_order_explicit_in_core_fragment_without_await(
    fragment: BlockPyCfgFragment<
        StructuredBlockPyStmtFor<CoreBlockPyExprWithYield>,
        BlockPyTerm<CoreBlockPyExprWithYield>,
    >,
) -> BlockPyCfgFragment<
    StructuredBlockPyStmtFor<CoreBlockPyExprWithYield>,
    BlockPyTerm<CoreBlockPyExprWithYield>,
> {
    let mut body = Vec::new();
    for stmt in fragment.body {
        make_eval_order_explicit_in_core_stmt_without_await(stmt, &mut body);
    }
    let term = fragment
        .term
        .map(|term| make_eval_order_explicit_in_core_term_without_await(term, &mut body));
    BlockPyCfgFragment { body, term }
}

fn make_eval_order_explicit_in_core_term_without_await(
    term: BlockPyTerm<CoreBlockPyExprWithYield>,
    out: &mut Vec<StructuredBlockPyStmtFor<CoreBlockPyExprWithYield>>,
) -> BlockPyTerm<CoreBlockPyExprWithYield> {
    match term {
        BlockPyTerm::Jump(_) => term,
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test,
            then_label,
            else_label,
        }) => BlockPyTerm::IfTerm(BlockPyIfTerm {
            test: hoist_core_expr_without_await_to_atom(test, out, &mut Vec::new()),
            then_label,
            else_label,
        }),
        BlockPyTerm::BranchTable(BlockPyBranchTable {
            index,
            targets,
            default_label,
        }) => BlockPyTerm::BranchTable(BlockPyBranchTable {
            index: hoist_core_expr_without_await_to_atom(index, out, &mut Vec::new()),
            targets,
            default_label,
        }),
        BlockPyTerm::Raise(BlockPyRaise { exc }) => BlockPyTerm::Raise(BlockPyRaise {
            exc: exc
                .map(|value| hoist_core_expr_without_await_to_atom(value, out, &mut Vec::new())),
        }),
        BlockPyTerm::Return(value) => BlockPyTerm::Return(hoist_core_expr_without_await_to_atom(
            value,
            out,
            &mut Vec::new(),
        )),
    }
}

pub(crate) fn make_eval_order_explicit_in_core_block_without_await(
    block: CfgBlock<
        StructuredBlockPyStmtFor<CoreBlockPyExprWithYield>,
        BlockPyTerm<CoreBlockPyExprWithYield>,
    >,
) -> CfgBlock<
    StructuredBlockPyStmtFor<CoreBlockPyExprWithYield>,
    BlockPyTerm<CoreBlockPyExprWithYield>,
> {
    let CfgBlock {
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
    CfgBlock {
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
        .map(|block| CfgBlock {
            label: block.label,
            body: block
                .body
                .into_iter()
                .map(|stmt| stmt.into_structured_stmt())
                .collect(),
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
