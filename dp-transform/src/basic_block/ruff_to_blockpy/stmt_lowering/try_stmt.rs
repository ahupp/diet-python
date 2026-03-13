use super::*;

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
