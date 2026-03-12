use super::*;

fn lower_nested_body_to_stmts(
    body: &StmtBody,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<BlockPyStmtFragment, String> {
    let mut out = BlockPyStmtFragmentBuilder::new();
    for stmt in &body.body {
        lower_stmt_into(stmt.as_ref(), &mut out, loop_ctx, next_label_id)?;
    }
    Ok(out.finish())
}

pub(crate) fn lower_stmt_into(
    stmt: &Stmt,
    out: &mut BlockPyStmtFragmentBuilder,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<(), String> {
    match stmt {
        Stmt::BodyStmt(body) => {
            for stmt in &body.body {
                lower_stmt_into(stmt.as_ref(), out, loop_ctx, next_label_id)?;
            }
        }
        Stmt::Global(_) | Stmt::Nonlocal(_) => {}
        Stmt::Pass(_) => out.push_stmt(BlockPyStmt::Pass),
        Stmt::Expr(expr_stmt) => {
            out.push_stmt(BlockPyStmt::Expr((*expr_stmt.value).clone().into()))
        }
        Stmt::Assign(assign) => {
            if assign.targets.len() != 1 {
                return Err(assign_delete_error(
                    "multi-target assignment reached BlockPy conversion",
                    stmt,
                ));
            }
            let Some(target) = assign.targets[0].as_name_expr().cloned() else {
                return Err(assign_delete_error(
                    "non-name assignment target reached BlockPy conversion",
                    stmt,
                ));
            };
            out.push_stmt(BlockPyStmt::Assign(BlockPyAssign {
                target,
                value: (*assign.value).clone().into(),
            }));
        }
        Stmt::Delete(delete) => {
            if delete.targets.len() != 1 {
                return Err(assign_delete_error(
                    "multi-target delete reached BlockPy conversion",
                    stmt,
                ));
            }
            let Some(target) = delete.targets[0].as_name_expr().cloned() else {
                return Err(assign_delete_error(
                    "non-name delete target reached BlockPy conversion",
                    stmt,
                ));
            };
            out.push_stmt(BlockPyStmt::Delete(BlockPyDelete { target }));
        }
        Stmt::FunctionDef(_) => {
            panic!("FunctionDef should be extracted before Ruff AST -> BlockPy conversion");
        }
        Stmt::ClassDef(_) => {
            panic!("ClassDef should be lowered before Ruff AST -> BlockPy conversion");
        }
        Stmt::TypeAlias(_) => {
            panic!("TypeAlias should be lowered before Ruff AST -> BlockPy conversion");
        }
        Stmt::AugAssign(_) => {
            panic!("AugAssign should be lowered before Ruff AST -> BlockPy conversion");
        }
        Stmt::AnnAssign(_) => {
            panic!("AnnAssign should be lowered before Ruff AST -> BlockPy conversion");
        }
        Stmt::If(if_stmt) => {
            let body = lower_nested_body_to_stmts(&if_stmt.body, loop_ctx, next_label_id)?;
            let orelse =
                lower_orelse_to_stmts(&if_stmt.elif_else_clauses, stmt, loop_ctx, next_label_id)?;
            out.push_stmt(BlockPyStmt::If(BlockPyIf {
                test: (*if_stmt.test).clone().into(),
                body,
                orelse,
            }));
        }
        Stmt::While(_) => {
            panic!("While should be lowered before Ruff AST -> BlockPy stmt-list conversion");
        }
        Stmt::For(_) => {
            panic!("For should be lowered before Ruff AST -> BlockPy stmt-list conversion");
        }
        Stmt::With(with_stmt) => {
            lower_with_into(with_stmt.clone(), out, loop_ctx, next_label_id)?;
        }
        Stmt::Match(_) => {
            panic!("Match should be lowered before Ruff AST -> BlockPy conversion");
        }
        Stmt::Assert(_) => {
            panic!("Assert should be lowered before Ruff AST -> BlockPy conversion");
        }
        Stmt::Import(_) => {
            panic!("Import should be lowered before Ruff AST -> BlockPy conversion");
        }
        Stmt::ImportFrom(_) => {
            panic!("ImportFrom should be lowered before Ruff AST -> BlockPy conversion");
        }
        Stmt::Break(_) => {
            if let Some(loop_ctx) = loop_ctx {
                out.set_term(BlockPyTerm::Jump(loop_ctx.break_label.clone()));
            } else {
                panic!("Break should be lowered before Ruff AST -> BlockPy conversion");
            }
        }
        Stmt::Continue(_) => {
            if let Some(loop_ctx) = loop_ctx {
                out.set_term(BlockPyTerm::Jump(loop_ctx.continue_label.clone()));
            } else {
                panic!("Continue should be lowered before Ruff AST -> BlockPy conversion");
            }
        }
        Stmt::Return(return_stmt) => {
            out.set_term(BlockPyTerm::Return(
                return_stmt.value.as_ref().map(|v| (**v).clone().into()),
            ));
        }
        Stmt::Raise(raise_stmt) => {
            if raise_stmt.cause.is_some() {
                panic!("raise-from should be lowered before Ruff AST -> BlockPy conversion");
            }
            out.set_term(BlockPyTerm::Raise(BlockPyRaise {
                exc: raise_stmt.exc.as_ref().map(|exc| (**exc).clone().into()),
            }));
        }
        Stmt::Try(_) => {
            panic!("Try should be lowered through stmt-sequence BlockPy conversion");
        }
        other => {
            return Err(format!(
                "unsupported statement reached Ruff AST -> BlockPy conversion: {}\nstmt:\n{}",
                stmt_kind_name(other),
                ruff_ast_to_string(other).trim_end()
            ));
        }
    }
    Ok(())
}

fn maybe_placeholder(expr: Expr) -> (Stmt, Expr, bool) {
    if is_simple(&expr) && !matches!(&expr, Expr::StringLiteral(_) | Expr::BytesLiteral(_)) {
        return (empty_body().into(), expr, false);
    }
    let tmp = fresh_name("tmp");
    let stmt = py_stmt!("{tmp:id} = {expr:expr}", tmp = tmp.as_str(), expr = expr);
    (stmt, py_expr!("{tmp:id}", tmp = tmp.as_str()), true)
}

fn with_target_object_expr(value: Expr) -> Expr {
    if let Expr::Name(name) = &value {
        py_expr!(
            "__dp_load_deleted_name({name:literal}, {value:expr})",
            name = name.id.as_str(),
            value = value,
        )
    } else {
        value
    }
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

pub(crate) fn lower_star_try_stmt_sequence<F>(
    try_stmt: ast::StmtTry,
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    jump_label: Option<String>,
    lower_sequence: &mut F,
) -> String
where
    F: FnMut(&[Box<Stmt>], String, &mut Vec<BlockPyBlock>) -> String,
{
    let rewritten_try = match rewrite_stmt::exception::rewrite_try(try_stmt) {
        Rewrite::Walk(stmt) | Rewrite::Unmodified(stmt) => stmt,
    };
    lower_expanded_stmt_sequence(
        rewritten_try,
        remaining_stmts,
        cont_label,
        linear,
        blocks,
        jump_label,
        lower_sequence,
    )
}

pub(crate) fn lower_try_stmt_sequence<F>(
    try_stmt: ast::StmtTry,
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    try_plan: TryPlan,
    lower_sequence: &mut F,
) -> (String, TryRegionPlan)
where
    F: FnMut(&[Box<Stmt>], String, &mut Vec<BlockPyBlock>) -> String,
{
    let rest_entry = lower_sequence(remaining_stmts, cont_label.clone(), blocks);

    let else_body = flatten_stmt_boxes(&try_stmt.orelse.body);
    let try_body = flatten_stmt_boxes(&try_stmt.body.body);
    let except_body =
        (!try_stmt.handlers.is_empty()).then(|| prepare_except_body(&try_stmt.handlers));
    let finally_body = if !try_stmt.finalbody.body.is_empty() {
        Some(prepare_finally_body(
            &try_stmt.finalbody,
            try_plan.finally_exc_name.as_deref(),
        ))
    } else {
        None
    };

    let lowered_try = lower_try_regions(
        blocks,
        &try_plan,
        rest_entry.as_str(),
        finally_body,
        else_body,
        try_body,
        except_body,
        lower_sequence,
    );

    finalize_try_regions(
        blocks,
        label,
        linear,
        lowered_try.body_label,
        lowered_try.except_label,
        try_plan,
        lowered_try.body_region_range,
        lowered_try.else_region_range,
        lowered_try.except_region_range,
        lowered_try.finally_region_range,
        lowered_try.finally_label,
        lowered_try.finally_normal_entry,
    )
}

fn rewrite_assignment_target<F>(target: Expr, rhs: Expr, out: &mut Vec<Stmt>, next_temp: &mut F)
where
    F: FnMut(&str) -> String,
{
    match target {
        Expr::Tuple(tuple) => rewrite_unpack_target(tuple.elts, rhs, out, next_temp),
        Expr::List(list) => rewrite_unpack_target(list.elts, rhs, out, next_temp),
        Expr::Subscript(ast::ExprSubscript { value, slice, .. }) => {
            out.push(py_stmt!(
                "__dp_setitem({obj:expr}, {key:expr}, {rhs:expr})",
                obj = with_target_object_expr(*value),
                key = *slice,
                rhs = rhs,
            ));
        }
        Expr::Attribute(ast::ExprAttribute { value, attr, .. }) => {
            out.push(py_stmt!(
                "__dp_setattr({obj:expr}, {name:literal}, {rhs:expr})",
                obj = with_target_object_expr(*value),
                name = attr.as_str(),
                rhs = rhs,
            ));
        }
        Expr::Name(ast::ExprName { id, .. }) => {
            out.push(py_stmt!(
                "{name:id} = {rhs:expr}",
                name = id.as_str(),
                rhs = rhs
            ));
        }
        other => {
            panic!("unsupported assignment target in Ruff AST -> BlockPy lowering: {other:?}");
        }
    }
}

fn rewrite_unpack_target<F>(elts: Vec<Expr>, value: Expr, out: &mut Vec<Stmt>, next_temp: &mut F)
where
    F: FnMut(&str) -> String,
{
    let unpacked_name = next_temp("tmp");
    let unpacked_tmp = py_expr!("{tmp:id}", tmp = unpacked_name.as_str());

    let mut spec_elts = Vec::new();
    let mut starred_seen = false;
    for elt in &elts {
        match elt {
            Expr::Starred(_) => {
                if starred_seen {
                    panic!("unsupported starred with-target assignment");
                }
                starred_seen = true;
                spec_elts.push(py_expr!("False"));
            }
            _ => spec_elts.push(py_expr!("True")),
        }
    }

    out.push(py_stmt!(
        "{tmp:id} = __dp_unpack({value:expr}, {spec:expr})",
        tmp = unpacked_name.as_str(),
        value = value,
        spec = make_tuple(spec_elts),
    ));

    let starred_index = elts.iter().position(|elt| matches!(elt, Expr::Starred(_)));
    for (idx, elt) in elts.into_iter().enumerate() {
        match elt {
            Expr::Starred(ast::ExprStarred { value, .. }) if Some(idx) == starred_index => {
                rewrite_assignment_target(
                    *value,
                    py_expr!(
                        "__dp_list(__dp_getitem({tmp:expr}, {idx:literal}))",
                        tmp = unpacked_tmp.clone(),
                        idx = idx as i64,
                    ),
                    out,
                    next_temp,
                );
            }
            other => {
                rewrite_assignment_target(
                    other,
                    py_expr!(
                        "__dp_getitem({tmp:expr}, {idx:literal})",
                        tmp = unpacked_tmp.clone(),
                        idx = idx as i64,
                    ),
                    out,
                    next_temp,
                );
            }
        }
    }

    out.push(py_stmt!("del {tmp:id}", tmp = unpacked_name.as_str()));
}

pub(crate) fn build_for_target_assign_body<F>(
    target: &Expr,
    tmp_expr: Expr,
    tmp_name: &str,
    cell_slots: &std::collections::HashSet<String>,
    next_temp: &mut F,
) -> Vec<Stmt>
where
    F: FnMut(&str) -> String,
{
    let mut out = Vec::new();
    rewrite_assignment_target(target.clone(), tmp_expr, &mut out, next_temp);
    out.extend(sync_target_cells_stmts_shared(target, cell_slots));
    out.push(py_stmt!("{tmp:id} = None", tmp = tmp_name));
    out
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

fn desugar_structured_with_stmt_for_blockpy(with_stmt: ast::StmtWith) -> Stmt {
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

        // Transitional desugaring: keep with semantics stable while Ruff AST ->
        // BlockPy owns this lowering. The long-term goal is a more direct BlockPy
        // representation that does not need the ok/suppress bookkeeping temps.
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

fn lower_with_into(
    with_stmt: ast::StmtWith,
    out: &mut BlockPyStmtFragmentBuilder,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<(), String> {
    let lowered_body = desugar_structured_with_stmt_for_blockpy(with_stmt);
    lower_stmt_into(&lowered_body, out, loop_ctx, next_label_id)
}

fn lower_orelse_to_stmts(
    clauses: &[ast::ElifElseClause],
    stmt: &Stmt,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<BlockPyStmtFragment, String> {
    match clauses {
        [] => Ok(BlockPyStmtFragment::from_stmts(Vec::new())),
        [clause] if clause.test.is_none() => {
            lower_nested_body_to_stmts(&clause.body, loop_ctx, next_label_id)
        }
        _ => Err(format!(
            "`elif` chain reached Ruff AST -> BlockPy conversion\nstmt:\n{}",
            ruff_ast_to_string(stmt).trim_end()
        )),
    }
}
