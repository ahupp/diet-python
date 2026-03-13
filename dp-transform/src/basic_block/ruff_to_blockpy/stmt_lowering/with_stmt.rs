use super::assign_stmt::{build_for_target_assign_body, rewrite_assignment_target};
use super::*;

impl StmtLowerer for ast::StmtWith {
    fn simplify_ast(self, _context: &Context) -> Stmt {
        desugar_structured_with_stmt_for_blockpy(self)
    }

    fn plan_head(self, _context: &Context, _allow_generator_head: bool) -> StmtSequenceHeadPlan {
        StmtSequenceHeadPlan::With(self)
    }

    fn to_blockpy<E>(
        &self,
        context: &Context,
        out: &mut BlockPyStmtFragmentBuilder<E>,
        loop_ctx: Option<&LoopContext>,
        next_label_id: &mut usize,
    ) -> Result<(), String>
    where
        E: From<Expr> + std::fmt::Debug,
    {
        lower_stmt_via_simplify(context, self, out, loop_ctx, next_label_id)
    }
}

fn maybe_placeholder(expr: Expr) -> (Stmt, Expr, bool) {
    if is_simple(&expr) && !matches!(&expr, Expr::StringLiteral(_) | Expr::BytesLiteral(_)) {
        return (empty_body().into(), expr, false);
    }
    let tmp = fresh_name("tmp");
    let stmt = py_stmt!("{tmp:id} = {expr:expr}", tmp = tmp.as_str(), expr = expr);
    (stmt, py_expr!("{tmp:id}", tmp = tmp.as_str()), true)
}

fn wrap_with_item_stmt(item: ast::WithItem, body: Stmt, is_async: bool) -> Stmt {
    let ast::WithItem {
        context_expr,
        optional_vars,
        ..
    } = item;
    match (is_async, optional_vars.map(|target| *target)) {
        (true, Some(target)) => py_stmt!(
            "async with {ctx:expr} as {target:expr}:\n    {body:stmt}",
            ctx = context_expr,
            target = target,
            body = body,
        ),
        (true, None) => py_stmt!(
            "async with {ctx:expr}:\n    {body:stmt}",
            ctx = context_expr,
            body = body,
        ),
        (false, Some(target)) => py_stmt!(
            "with {ctx:expr} as {target:expr}:\n    {body:stmt}",
            ctx = context_expr,
            target = target,
            body = body,
        ),
        (false, None) => py_stmt!(
            "with {ctx:expr}:\n    {body:stmt}",
            ctx = context_expr,
            body = body,
        ),
    }
}

fn nest_with_stmt_items(with_stmt: ast::StmtWith) -> Stmt {
    let ast::StmtWith {
        items,
        body,
        is_async,
        ..
    } = with_stmt;
    items
        .into_iter()
        .rev()
        .fold(Stmt::BodyStmt(body), |body, item| {
            wrap_with_item_stmt(item, body, is_async)
        })
}

pub(super) fn desugar_structured_with_stmt_for_blockpy(with_stmt: ast::StmtWith) -> Stmt {
    if with_stmt.items.is_empty() {
        return Stmt::BodyStmt(with_stmt.body);
    }

    let ast::StmtWith {
        items,
        body,
        is_async,
        ..
    } = with_stmt;

    let mut lowered_body: Stmt = body.into();

    for ast::WithItem {
        context_expr,
        optional_vars,
        ..
    } in items.into_iter().rev()
    {
        let target = optional_vars.map(|var| *var);
        let exit_name = fresh_name("with_exit");
        let ok_name = fresh_name("with_ok");
        let reraise_name = fresh_name("with_reraise");
        let (ctx_placeholder_stmt, ctx_expr, ctx_was_placeholder) = maybe_placeholder(context_expr);
        let ctx_cleanup = if ctx_was_placeholder {
            py_stmt!("{ctx:expr} = None", ctx = ctx_expr.clone())
        } else {
            empty_body().into()
        };

        let enter_value = if is_async {
            py_expr!(
                "await __dp_asynccontextmanager_aenter({ctx:expr})",
                ctx = ctx_expr.clone()
            )
        } else {
            py_expr!(
                "__dp_contextmanager_enter({ctx:expr})",
                ctx = ctx_expr.clone()
            )
        };
        let enter_stmt = if let Some(target) = target.clone() {
            let mut enter_stmts = Vec::new();
            let mut next_temp = |prefix: &str| fresh_name(prefix);
            rewrite_assignment_target(target, enter_value, &mut enter_stmts, &mut next_temp);
            into_body(enter_stmts)
        } else {
            py_stmt!("{value:expr}", value = enter_value)
        };

        lowered_body = if is_async {
            py_stmt!(
                r#"
{ctx_placeholder_stmt:stmt}
{exit_name:id} = __dp_asynccontextmanager_get_aexit({ctx_expr:expr})
{enter_stmt:stmt}
{ok_name:id} = True
try:
    {body:stmt}
except BaseException:
    {ok_name:id} = False
    {reraise_name:id} = await __dp_asynccontextmanager_exit({exit_name:id}, __dp_exc_info())
    if __dp_is_not({reraise_name:id}, None):
        raise {reraise_name:id}
finally:
    if {ok_name:id}:
        await __dp_asynccontextmanager_exit({exit_name:id}, None)
    {exit_name:id} = None
    {ctx_cleanup:stmt}
"#,
                ctx_placeholder_stmt = ctx_placeholder_stmt,
                ctx_expr = ctx_expr.clone(),
                enter_stmt = enter_stmt,
                body = lowered_body,
                exit_name = exit_name.as_str(),
                ok_name = ok_name.as_str(),
                reraise_name = reraise_name.as_str(),
                ctx_cleanup = ctx_cleanup,
            )
        } else {
            py_stmt!(
                r#"
{ctx_placeholder_stmt:stmt}
{exit_name:id} = __dp_contextmanager_get_exit({ctx_expr:expr})
{enter_stmt:stmt}
{ok_name:id} = True
try:
    {body:stmt}
except BaseException:
    {ok_name:id} = False
    __dp_contextmanager_exit({exit_name:id}, __dp_exc_info())
finally:
    if {ok_name:id}:
        __dp_contextmanager_exit({exit_name:id}, None)
    {exit_name:id} = None
    {ctx_cleanup:stmt}
"#,
                ctx_placeholder_stmt = ctx_placeholder_stmt,
                ctx_expr = ctx_expr.clone(),
                enter_stmt = enter_stmt,
                body = lowered_body,
                exit_name = exit_name.as_str(),
                ok_name = ok_name.as_str(),
                ctx_cleanup = ctx_cleanup,
            )
        };
    }

    if is_async {
        lower_coroutine_awaits_in_stmt(lowered_body)
    } else {
        lowered_body
    }
}

fn build_with_finally_body(
    exit_name: &str,
    exit_call_name: &str,
    enter_name: Option<&str>,
    ctx_cleanup: Stmt,
    is_async: bool,
) -> Vec<Box<Stmt>> {
    let mut out = vec![
        Box::new(py_stmt!(
            "{exit_call:id} = {exit:id}",
            exit_call = exit_call_name,
            exit = exit_name,
        )),
        Box::new(py_stmt!("{exit:id} = None", exit = exit_name)),
    ];
    if let Some(enter_name) = enter_name {
        out.push(Box::new(py_stmt!("{enter:id} = None", enter = enter_name)));
    }
    if !matches!(&ctx_cleanup, Stmt::BodyStmt(ast::StmtBody { body, .. }) if body.is_empty()) {
        out.push(Box::new(ctx_cleanup));
    }
    let exit_stmt = if is_async {
        py_stmt!(
            "await __dp_asynccontextmanager_exit({exit_call:id}, __dp_exc_info())",
            exit_call = exit_call_name,
        )
    } else {
        py_stmt!(
            "__dp_contextmanager_exit({exit_call:id}, __dp_exc_info())",
            exit_call = exit_call_name,
        )
    };
    out.push(Box::new(exit_stmt));
    out.push(Box::new(py_stmt!(
        "{exit_call:id} = None",
        exit_call = exit_call_name,
    )));
    out
}

pub(crate) fn lower_with_stmt_sequence<F>(
    fn_name: &str,
    with_stmt: ast::StmtWith,
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    cell_slots: &HashSet<String>,
    next_block_id: &Cell<usize>,
    needs_finally_return_flow: bool,
    lower_sequence: &mut F,
) -> (String, Option<TryRegionPlan>)
where
    F: FnMut(&[Box<Stmt>], String, &mut Vec<BlockPyBlock>) -> String,
{
    if with_stmt.items.is_empty() {
        let jump_label = if linear.is_empty() {
            None
        } else {
            let mut next_id = next_block_id.get();
            let label = compat_next_label(fn_name, &mut next_id);
            next_block_id.set(next_id);
            Some(label)
        };
        return (
            lower_expanded_stmt_sequence(
                Stmt::BodyStmt(with_stmt.body),
                remaining_stmts,
                cont_label,
                linear,
                blocks,
                jump_label,
                lower_sequence,
            ),
            None,
        );
    }

    if with_stmt.is_async {
        let jump_label = if linear.is_empty() {
            None
        } else {
            let mut next_id = next_block_id.get();
            let label = compat_next_label(fn_name, &mut next_id);
            next_block_id.set(next_id);
            Some(label)
        };
        return (
            lower_expanded_stmt_sequence(
                desugar_structured_with_stmt_for_blockpy(with_stmt),
                remaining_stmts,
                cont_label,
                linear,
                blocks,
                jump_label,
                lower_sequence,
            ),
            None,
        );
    }

    if with_stmt.items.len() != 1 {
        let jump_label = if linear.is_empty() {
            None
        } else {
            let mut next_id = next_block_id.get();
            let label = compat_next_label(fn_name, &mut next_id);
            next_block_id.set(next_id);
            Some(label)
        };
        return (
            lower_expanded_stmt_sequence(
                nest_with_stmt_items(with_stmt),
                remaining_stmts,
                cont_label,
                linear,
                blocks,
                jump_label,
                lower_sequence,
            ),
            None,
        );
    }

    let ast::StmtWith {
        mut items,
        body,
        is_async,
        ..
    } = with_stmt;
    let ast::WithItem {
        context_expr,
        optional_vars,
        ..
    } = items.pop().expect("single-item with should have one item");

    let rest_entry = lower_sequence(remaining_stmts, cont_label.clone(), blocks);
    let mut next_id = next_block_id.get();
    let try_plan = build_try_plan(fn_name, true, needs_finally_return_flow, &mut next_id);
    let label = compat_next_label(fn_name, &mut next_id);
    let exit_name = compat_next_temp("with_exit", &mut next_id);
    let exit_call_name = compat_next_temp("with_exit_call", &mut next_id);
    next_block_id.set(next_id);
    let (ctx_placeholder_stmt, ctx_expr, ctx_was_placeholder) = maybe_placeholder(context_expr);
    let ctx_cleanup = if ctx_was_placeholder {
        py_stmt!("{ctx:expr} = None", ctx = ctx_expr.clone())
    } else {
        empty_body().into()
    };

    let mut entry_linear = linear;
    if !matches!(&ctx_placeholder_stmt, Stmt::BodyStmt(ast::StmtBody { body, .. }) if body.is_empty())
    {
        entry_linear.push(ctx_placeholder_stmt);
    }

    let exit_lookup = if is_async {
        py_stmt!(
            "{exit:id} = __dp_asynccontextmanager_get_aexit({ctx:expr})",
            exit = exit_name.as_str(),
            ctx = ctx_expr.clone(),
        )
    } else {
        py_stmt!(
            "{exit:id} = __dp_contextmanager_get_exit({ctx:expr})",
            exit = exit_name.as_str(),
            ctx = ctx_expr.clone(),
        )
    };
    entry_linear.push(exit_lookup);

    let enter_value = if is_async {
        py_expr!(
            "await __dp_asynccontextmanager_aenter({ctx:expr})",
            ctx = ctx_expr.clone(),
        )
    } else {
        py_expr!(
            "__dp_contextmanager_enter({ctx:expr})",
            ctx = ctx_expr.clone(),
        )
    };

    let mut try_body = Vec::new();
    let enter_name = optional_vars.as_ref().map(|_| {
        let mut next_id = next_block_id.get();
        let name = compat_next_temp("with_enter", &mut next_id);
        next_block_id.set(next_id);
        name
    });
    if let Some(target) = optional_vars.as_ref().map(|target| target.as_ref()) {
        let enter_name = enter_name
            .as_ref()
            .expect("with target should reserve enter temp")
            .clone();
        entry_linear.push(py_stmt!(
            "{enter:id} = {value:expr}",
            enter = enter_name.as_str(),
            value = enter_value,
        ));
        try_body.extend(
            build_for_target_assign_body(
                target,
                py_expr!("{name:id}", name = enter_name.as_str()),
                enter_name.as_str(),
                cell_slots,
                &mut |prefix| {
                    let mut next_id = next_block_id.get();
                    let name = compat_next_temp(prefix, &mut next_id);
                    next_block_id.set(next_id);
                    name
                },
            )
            .into_iter()
            .map(Box::new),
        );
    } else {
        entry_linear.push(py_stmt!("{value:expr}", value = enter_value));
    }
    try_body.extend(flatten_stmt_boxes(&body.body));

    let finally_body = build_with_finally_body(
        exit_name.as_str(),
        exit_call_name.as_str(),
        enter_name.as_deref(),
        ctx_cleanup,
        is_async,
    );
    let lowered_with = lower_try_regions(
        blocks,
        &try_plan,
        rest_entry.as_str(),
        Some(finally_body),
        Vec::new(),
        try_body,
        None,
        lower_sequence,
    );
    let (entry, try_region) = finalize_try_regions(
        blocks,
        label,
        entry_linear,
        lowered_with.body_label,
        lowered_with.except_label,
        try_plan,
        lowered_with.body_region_range,
        lowered_with.else_region_range,
        lowered_with.except_region_range,
        lowered_with.finally_region_range,
        lowered_with.finally_label,
        lowered_with.finally_normal_entry,
    );
    (entry, Some(try_region))
}

#[cfg(test)]
mod tests {
    use super::super::{simplify_stmt_ast_for_blockpy, BlockPyStmtFragmentBuilder};
    use super::*;
    use crate::basic_block::ast_to_ast::{context::Context, Options};

    #[test]
    fn stmt_with_simplify_ast_desugars_before_blockpy_lowering() {
        let stmt = py_stmt!("with cm:\n    body()");
        let Stmt::With(with_stmt) = stmt else {
            panic!("expected with stmt");
        };

        let context = Context::new(Options::for_test(), "");
        let simplified = simplify_stmt_ast_for_blockpy(&context, Stmt::With(with_stmt));

        assert!(!matches!(simplified, Stmt::With(_)));
    }

    #[test]
    #[should_panic(expected = "StmtTry should have already been reduced before BlockPy lowering")]
    fn stmt_with_to_blockpy_simplifies_before_hitting_sequence_only_try_lowering() {
        let stmt = py_stmt!("with cm:\n    body()");
        let Stmt::With(with_stmt) = stmt else {
            panic!("expected with stmt");
        };
        let context = Context::new(Options::for_test(), "");
        let mut out = BlockPyStmtFragmentBuilder::<BlockPyExpr>::new();
        let mut next_label_id = 0usize;

        let _ = with_stmt.to_blockpy(&context, &mut out, None, &mut next_label_id);
    }
}
