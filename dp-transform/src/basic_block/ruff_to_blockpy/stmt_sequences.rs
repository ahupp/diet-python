use super::stmt_lowering::lower_stmt_into_with_expr;
use super::*;
use crate::basic_block::annotation_export::build_exec_function_def_binding_stmts;
use crate::basic_block::ast_to_ast::body::suite_ref;
use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::block_py::{BlockPyBlock, BlockPyRaise, BlockPyStmt, BlockPyTerm, Expr};

pub(crate) fn lower_stmts_to_blockpy_stmts_with_context<E>(
    context: &Context,
    stmts: &[Stmt],
) -> Result<crate::basic_block::block_py::BlockPyCfgFragment<BlockPyStmt<E>, BlockPyTerm<E>>, String>
where
    E: From<Expr> + std::fmt::Debug,
{
    let mut out = crate::basic_block::block_py::BlockPyCfgFragmentBuilder::<
        BlockPyStmt<E>,
        BlockPyTerm<E>,
    >::new();
    let mut next_label_id = 0usize;
    for stmt in stmts {
        lower_stmt_into_with_expr(context, stmt, &mut out, None, &mut next_label_id)?;
    }
    Ok(out.finish())
}

pub(crate) fn lower_stmts_to_blockpy_stmts<E>(
    stmts: &[Stmt],
) -> Result<crate::basic_block::block_py::BlockPyCfgFragment<BlockPyStmt<E>, BlockPyTerm<E>>, String>
where
    E: From<Expr> + std::fmt::Debug,
{
    let context = Context::new(crate::basic_block::ast_to_ast::Options::for_test(), "");
    lower_stmts_to_blockpy_stmts_with_context(&context, stmts)
}

pub(crate) fn plan_stmt_sequence_head(context: &Context, stmt: &Stmt) -> StmtSequenceHeadPlan {
    super::stmt_lowering::plan_stmt_head_for_blockpy(context, stmt)
}

pub(crate) fn drive_stmt_sequence_until_control<FDelete>(
    context: &Context,
    stmts: &[Stmt],
    mut linear: Vec<Stmt>,
    cell_slots: &HashSet<String>,
    outer_scope_names: &HashSet<String>,
    rewrite_delete: &mut FDelete,
) -> StmtSequenceDriveResult
where
    FDelete: FnMut(&ast::StmtDelete) -> Vec<Stmt>,
{
    let mut index = 0;
    while index < stmts.len() {
        match plan_stmt_sequence_head(context, &stmts[index]) {
            StmtSequenceHeadPlan::Linear(stmt) => {
                linear.push(stmt);
                index += 1;
            }
            StmtSequenceHeadPlan::Expanded(stmts) => {
                return StmtSequenceDriveResult::Break {
                    linear,
                    index,
                    plan: StmtSequenceHeadPlan::Expanded(stmts),
                };
            }
            StmtSequenceHeadPlan::FunctionDef(func_def) => {
                if func_def.name.id.as_str().starts_with("_dp_bb_") {
                    linear.push(Stmt::FunctionDef(func_def));
                } else {
                    linear.extend(build_exec_function_def_binding_stmts(
                        &func_def,
                        cell_slots,
                        outer_scope_names,
                    ));
                }
                index += 1;
            }
            StmtSequenceHeadPlan::Delete(delete_stmt) => {
                linear.extend(rewrite_delete(&delete_stmt));
                index += 1;
            }
            plan => {
                return StmtSequenceDriveResult::Break {
                    linear,
                    index,
                    plan,
                };
            }
        }
    }
    StmtSequenceDriveResult::Exhausted { linear }
}

fn compat_blockpy_raise_from_stmt(raise_stmt: ast::StmtRaise) -> BlockPyRaise {
    assert!(
        raise_stmt.cause.is_none(),
        "raise-from should be lowered before Ruff AST -> BlockPy conversion"
    );
    BlockPyRaise {
        exc: raise_stmt.exc.map(|expr| (*expr).into()),
    }
}

fn rewrite_delete_to_deleted_sentinel(delete_stmt: &ast::StmtDelete) -> Vec<Stmt> {
    let mut out = Vec::new();
    for target in &delete_stmt.targets {
        rewrite_delete_target_to_deleted_sentinel(target, &mut out);
    }
    out
}

fn rewrite_delete_target_to_deleted_sentinel(target: &Expr, out: &mut Vec<Stmt>) {
    match target {
        Expr::Name(name) => {
            out.push(py_stmt!(
                "{name:id} = __dp_DELETED",
                name = name.id.as_str(),
            ));
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                rewrite_delete_target_to_deleted_sentinel(elt, out);
            }
        }
        Expr::List(list) => {
            for elt in &list.elts {
                rewrite_delete_target_to_deleted_sentinel(elt, out);
            }
        }
        Expr::Starred(starred) => {
            rewrite_delete_target_to_deleted_sentinel(starred.value.as_ref(), out);
        }
        _ => out.push(py_stmt!("del {target:expr}", target = target.clone())),
    }
}

pub(crate) fn lower_common_stmt_sequence_head<FSeq>(
    context: &Context,
    plan: StmtSequenceHeadPlan,
    remaining_stmts: &[Stmt],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    next_label: &mut dyn FnMut() -> String,
    break_label: Option<String>,
    continue_label: Option<String>,
    lower_sequence: &mut FSeq,
) -> Option<String>
where
    FSeq: FnMut(&[Stmt], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    match plan {
        StmtSequenceHeadPlan::Raise(raise_stmt) => Some(
            emit_sequence_raise_block_with_expr_setup(
                context,
                blocks,
                next_label(),
                linear,
                compat_blockpy_raise_from_stmt(raise_stmt),
            )
            .unwrap_or_else(|err| {
                panic!("failed to lower sequence raise head through expr seam: {err}")
            }),
        ),
        StmtSequenceHeadPlan::Return(value) => Some(
            emit_sequence_return_block_with_expr_setup(
                context,
                blocks,
                next_label(),
                linear,
                value,
            )
            .unwrap_or_else(|err| {
                panic!("failed to lower sequence return head through expr seam: {err}")
            }),
        ),
        StmtSequenceHeadPlan::If(if_stmt) => Some(lower_if_stmt_sequence_from_stmt(
            context,
            if_stmt,
            remaining_stmts,
            cont_label,
            linear,
            blocks,
            next_label(),
            &mut |stmts, cont_label, blocks| lower_sequence(stmts, cont_label, None, blocks),
        )),
        StmtSequenceHeadPlan::While(while_stmt) => {
            let test_label = next_label();
            let linear_label = if linear.is_empty() {
                None
            } else {
                Some(next_label())
            };
            Some(lower_while_stmt_sequence_from_stmt(
                context,
                while_stmt,
                remaining_stmts,
                cont_label,
                linear,
                blocks,
                test_label,
                linear_label,
                lower_sequence,
            ))
        }
        StmtSequenceHeadPlan::Break => match break_label {
            Some(break_label) => Some(emit_sequence_jump_block(
                blocks,
                next_label(),
                linear,
                break_label,
            )),
            None => Some(cont_label),
        },
        StmtSequenceHeadPlan::Continue => match continue_label {
            Some(continue_label) => Some(emit_sequence_jump_block(
                blocks,
                next_label(),
                linear,
                continue_label,
            )),
            None => Some(cont_label),
        },
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_for_stmt_sequence_head<F>(
    fn_name: &str,
    for_stmt: ast::StmtFor,
    remaining_stmts: &[Stmt],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    iter_name: &str,
    tmp_name: &str,
    loop_check_label: String,
    loop_continue_label: String,
    assign_body: Vec<Stmt>,
    next_block_id: &Cell<usize>,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Stmt], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let mut next_id = next_block_id.get();
    let assign_label = compat_next_label(fn_name, &mut next_id);
    let setup_label = compat_next_label(fn_name, &mut next_id);
    next_block_id.set(next_id);
    lower_for_stmt_sequence(
        for_stmt,
        remaining_stmts,
        cont_label,
        linear,
        blocks,
        iter_name,
        tmp_name,
        loop_check_label,
        loop_continue_label,
        assign_label,
        setup_label,
        assign_body,
        lower_region,
    )
}

pub(crate) fn lower_stmt_sequence_with_state<FTemp>(
    context: &Context,
    fn_name: &str,
    stmts: &[Stmt],
    cont_label: String,
    break_label: Option<String>,
    continue_label: Option<String>,
    blocks: &mut Vec<BlockPyBlock>,
    cell_slots: &HashSet<String>,
    outer_scope_names: &HashSet<String>,
    try_regions: &mut Vec<TryRegionPlan>,
    next_block_id: &mut usize,
    next_temp: &mut FTemp,
) -> String
where
    FTemp: FnMut(&str, &mut usize) -> String,
{
    if stmts.is_empty() {
        return cont_label;
    }

    let mut linear = Vec::new();
    let mut index = 0;
    while index < stmts.len() {
        let plan;
        (linear, index, plan) = match drive_stmt_sequence_until_control(
            context,
            &stmts[index..],
            linear,
            cell_slots,
            outer_scope_names,
            &mut rewrite_delete_to_deleted_sentinel,
        ) {
            StmtSequenceDriveResult::Exhausted { linear } => {
                let label = compat_next_label(fn_name, next_block_id);
                return emit_sequence_jump_block(blocks, label, linear, cont_label);
            }
            StmtSequenceDriveResult::Break {
                linear,
                index: break_index,
                plan,
            } => (linear, index + break_index, plan),
        };

        match plan {
            plan @ (StmtSequenceHeadPlan::Raise(_)
            | StmtSequenceHeadPlan::Return(_)
            | StmtSequenceHeadPlan::If(_)
            | StmtSequenceHeadPlan::While(_)
            | StmtSequenceHeadPlan::Break
            | StmtSequenceHeadPlan::Continue) => {
                let next_id = Cell::new(*next_block_id);
                let label = lower_common_stmt_sequence_head(
                    context,
                    plan,
                    &stmts[index + 1..],
                    cont_label.clone(),
                    linear,
                    blocks,
                    &mut || {
                        let mut local_next_id = next_id.get();
                        let label = compat_next_label(fn_name, &mut local_next_id);
                        next_id.set(local_next_id);
                        label
                    },
                    break_label.clone(),
                    continue_label.clone(),
                    &mut |stmts, cont_label, loop_break_label, blocks| {
                        if let Some(loop_break_label) = loop_break_label {
                            let mut local_next_id = next_id.get();
                            let label = lower_stmt_sequence_with_state(
                                context,
                                fn_name,
                                stmts,
                                cont_label.clone(),
                                Some(loop_break_label),
                                Some(cont_label),
                                blocks,
                                cell_slots,
                                outer_scope_names,
                                try_regions,
                                &mut local_next_id,
                                next_temp,
                            );
                            next_id.set(local_next_id);
                            label
                        } else {
                            let mut local_next_id = next_id.get();
                            let label = lower_stmt_sequence_with_state(
                                context,
                                fn_name,
                                stmts,
                                cont_label,
                                break_label.clone(),
                                continue_label.clone(),
                                blocks,
                                cell_slots,
                                outer_scope_names,
                                try_regions,
                                &mut local_next_id,
                                next_temp,
                            );
                            next_id.set(local_next_id);
                            label
                        }
                    },
                );
                *next_block_id = next_id.into_inner();
                if let Some(label) = label {
                    return label;
                }
                unreachable!("common head helper must lower supported head");
            }
            StmtSequenceHeadPlan::With(with_stmt) => {
                let needs_finally_return_flow =
                    contains_return_stmt_in_body(suite_ref(&with_stmt.body));
                let next_id = Cell::new(*next_block_id);
                let (entry, try_region) = lower_with_stmt_sequence(
                    fn_name,
                    with_stmt,
                    &stmts[index + 1..],
                    cont_label.clone(),
                    linear,
                    blocks,
                    cell_slots,
                    &next_id,
                    needs_finally_return_flow,
                    &mut |stmts, cont_label, blocks| {
                        let mut local_next_id = next_id.get();
                        let label = lower_stmt_sequence_with_state(
                            context,
                            fn_name,
                            stmts,
                            cont_label,
                            break_label.clone(),
                            continue_label.clone(),
                            blocks,
                            cell_slots,
                            outer_scope_names,
                            try_regions,
                            &mut local_next_id,
                            next_temp,
                        );
                        next_id.set(local_next_id);
                        label
                    },
                );
                *next_block_id = next_id.into_inner();
                if let Some(try_region) = try_region {
                    try_regions.push(try_region);
                }
                return entry;
            }
            StmtSequenceHeadPlan::For(for_stmt) => {
                let iter_name = next_temp("iter", next_block_id);
                let tmp_name = next_temp("tmp", next_block_id);
                let tmp_expr = py_expr!("{name:id}", name = tmp_name.as_str());
                let loop_check_label = compat_next_label(fn_name, next_block_id);
                let loop_continue_label = loop_check_label.clone();
                let assign_body = build_for_target_assign_body(
                    for_stmt.target.as_ref(),
                    tmp_expr,
                    tmp_name.as_str(),
                    cell_slots,
                    &mut |prefix| next_temp(prefix, next_block_id),
                );
                let next_id = Cell::new(*next_block_id);
                let label = lower_for_stmt_sequence_head(
                    fn_name,
                    for_stmt,
                    &stmts[index + 1..],
                    cont_label.clone(),
                    linear,
                    blocks,
                    iter_name.as_str(),
                    tmp_name.as_str(),
                    loop_check_label,
                    loop_continue_label,
                    assign_body,
                    &next_id,
                    &mut |stmts, cont_label, loop_break_label, blocks| {
                        if let Some(loop_break_label) = loop_break_label {
                            let mut local_next_id = next_id.get();
                            let label = lower_stmt_sequence_with_state(
                                context,
                                fn_name,
                                stmts,
                                cont_label.clone(),
                                Some(loop_break_label),
                                Some(cont_label),
                                blocks,
                                cell_slots,
                                outer_scope_names,
                                try_regions,
                                &mut local_next_id,
                                next_temp,
                            );
                            next_id.set(local_next_id);
                            label
                        } else {
                            let mut local_next_id = next_id.get();
                            let label = lower_stmt_sequence_with_state(
                                context,
                                fn_name,
                                stmts,
                                cont_label,
                                break_label.clone(),
                                continue_label.clone(),
                                blocks,
                                cell_slots,
                                outer_scope_names,
                                try_regions,
                                &mut local_next_id,
                                next_temp,
                            );
                            next_id.set(local_next_id);
                            label
                        }
                    },
                );
                *next_block_id = next_id.into_inner();
                return label;
            }
            StmtSequenceHeadPlan::Try(try_stmt) => {
                let next_id = Cell::new(*next_block_id);
                let label = if try_stmt.is_star {
                    let mut local_next_id = next_id.get();
                    let jump_label = (!linear.is_empty())
                        .then(|| compat_next_label(fn_name, &mut local_next_id));
                    next_id.set(local_next_id);
                    lower_star_try_stmt_sequence(
                        try_stmt,
                        &stmts[index + 1..],
                        cont_label.clone(),
                        linear,
                        blocks,
                        jump_label,
                        &mut |stmts, cont_label, blocks| {
                            let mut local_next_id = next_id.get();
                            let label = lower_stmt_sequence_with_state(
                                context,
                                fn_name,
                                stmts,
                                cont_label,
                                break_label.clone(),
                                continue_label.clone(),
                                blocks,
                                cell_slots,
                                outer_scope_names,
                                try_regions,
                                &mut local_next_id,
                                next_temp,
                            );
                            next_id.set(local_next_id);
                            label
                        },
                    )
                } else {
                    let mut local_next_id = next_id.get();
                    let has_finally = !suite_ref(&try_stmt.finalbody).is_empty();
                    let needs_finally_return_flow = has_finally
                        && (contains_return_stmt_in_body(suite_ref(&try_stmt.body))
                            || contains_return_stmt_in_handlers(&try_stmt.handlers)
                            || contains_return_stmt_in_body(suite_ref(&try_stmt.orelse)));
                    let try_plan = build_try_plan(
                        fn_name,
                        has_finally,
                        needs_finally_return_flow,
                        &mut local_next_id,
                    );
                    let label = compat_next_label(fn_name, &mut local_next_id);
                    next_id.set(local_next_id);
                    let (entry, try_region) = lower_try_stmt_sequence(
                        try_stmt,
                        &stmts[index + 1..],
                        cont_label.clone(),
                        linear,
                        blocks,
                        label.clone(),
                        try_plan,
                        &mut |stmts, cont_label, blocks| {
                            let mut local_next_id = next_id.get();
                            let label = lower_stmt_sequence_with_state(
                                context,
                                fn_name,
                                stmts,
                                cont_label,
                                break_label.clone(),
                                continue_label.clone(),
                                blocks,
                                cell_slots,
                                outer_scope_names,
                                try_regions,
                                &mut local_next_id,
                                next_temp,
                            );
                            next_id.set(local_next_id);
                            label
                        },
                    );
                    try_regions.push(try_region);
                    entry
                };
                *next_block_id = next_id.into_inner();
                return label;
            }
            StmtSequenceHeadPlan::Linear(_)
            | StmtSequenceHeadPlan::FunctionDef(_)
            | StmtSequenceHeadPlan::Delete(_) => {
                unreachable!("sequence driver should consume linear/functiondef/delete heads")
            }
            StmtSequenceHeadPlan::Expanded(expanded_stmts) => {
                let jump_label =
                    (!linear.is_empty()).then(|| compat_next_label(fn_name, next_block_id));
                return lower_expanded_stmt_sequence(
                    expanded_stmts,
                    &stmts[index + 1..],
                    cont_label,
                    linear,
                    blocks,
                    jump_label,
                    &mut |stmts, cont_label, blocks| {
                        lower_stmt_sequence_with_state(
                            context,
                            fn_name,
                            stmts,
                            cont_label,
                            break_label.clone(),
                            continue_label.clone(),
                            blocks,
                            cell_slots,
                            outer_scope_names,
                            try_regions,
                            next_block_id,
                            next_temp,
                        )
                    },
                );
            }
            StmtSequenceHeadPlan::Unsupported => return cont_label,
        }
    }

    let label = compat_next_label(fn_name, next_block_id);
    emit_sequence_jump_block(blocks, label, linear, cont_label)
}

pub(crate) fn lower_expanded_stmt_sequence<F>(
    desugared_stmts: Vec<Stmt>,
    remaining_stmts: &[Stmt],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    jump_label: Option<String>,
    lower_sequence: &mut F,
) -> String
where
    F: FnMut(&[Stmt], String, &mut Vec<BlockPyBlock>) -> String,
{
    let mut expanded = desugared_stmts;
    expanded.extend_from_slice(remaining_stmts);
    let expanded_entry = lower_sequence(&expanded, cont_label, blocks);
    if linear.is_empty() {
        return expanded_entry;
    }
    let jump_label = jump_label.expect("linear prefix requires a jump label");
    blocks.push(compat_block_from_blockpy(
        jump_label.clone(),
        linear,
        BlockPyTerm::Jump(BlockPyLabel::from(expanded_entry).into()),
    ));
    jump_label
}

pub(crate) fn lower_if_stmt_sequence<F>(
    context: &Context,
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    linear: Vec<Stmt>,
    test: Expr,
    then_body: &[Stmt],
    else_body: &[Stmt],
    rest_entry: String,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Stmt], String, &mut Vec<BlockPyBlock>) -> String,
{
    let then_entry = lower_region(then_body, rest_entry.clone(), blocks);
    let else_entry = lower_region(else_body, rest_entry, blocks);
    emit_if_branch_block_with_expr_setup(
        context, blocks, label, linear, test, then_entry, else_entry,
    )
    .unwrap_or_else(|err| panic!("failed to lower sequence if head through expr seam: {err}"))
}

pub(crate) fn lower_if_stmt_sequence_from_stmt<F>(
    context: &Context,
    if_stmt: ast::StmtIf,
    remaining_stmts: &[Stmt],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Stmt], String, &mut Vec<BlockPyBlock>) -> String,
{
    let then_body = suite_ref(&if_stmt.body).to_vec();
    let else_body = extract_if_else_body(&if_stmt);
    let rest_entry = lower_region(remaining_stmts, cont_label, blocks);
    lower_if_stmt_sequence(
        context,
        blocks,
        label,
        linear,
        *if_stmt.test,
        &then_body,
        &else_body,
        rest_entry,
        lower_region,
    )
}

fn extract_if_else_body(if_stmt: &ast::StmtIf) -> Vec<Stmt> {
    if if_stmt.elif_else_clauses.is_empty() {
        return Vec::new();
    }
    if_stmt
        .elif_else_clauses
        .first()
        .map(|clause| suite_ref(&clause.body).to_vec())
        .unwrap_or_default()
}

pub(crate) fn lower_while_stmt_sequence<F>(
    context: &Context,
    blocks: &mut Vec<BlockPyBlock>,
    test_label: String,
    linear_label: Option<String>,
    linear: Vec<Stmt>,
    test: Expr,
    body: &[Stmt],
    else_body: &[Stmt],
    remaining_stmts: &[Stmt],
    cont_label: String,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Stmt], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let rest_entry = lower_region(remaining_stmts, cont_label, None, blocks);
    let cond_false_entry = if else_body.is_empty() {
        rest_entry.clone()
    } else {
        lower_region(else_body, rest_entry.clone(), None, blocks)
    };
    let body_entry = lower_region(body, test_label.clone(), Some(rest_entry), blocks);
    emit_simple_while_blocks_with_expr_setup(
        context,
        blocks,
        test_label,
        linear_label,
        linear,
        test,
        body_entry,
        cond_false_entry,
    )
    .unwrap_or_else(|err| panic!("failed to lower sequence while head through expr seam: {err}"))
}

pub(crate) fn lower_while_stmt_sequence_from_stmt<F>(
    context: &Context,
    while_stmt: ast::StmtWhile,
    remaining_stmts: &[Stmt],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    test_label: String,
    linear_label: Option<String>,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Stmt], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let body = suite_ref(&while_stmt.body).to_vec();
    let else_body = suite_ref(&while_stmt.orelse).to_vec();
    lower_while_stmt_sequence(
        context,
        blocks,
        test_label,
        linear_label,
        linear,
        *while_stmt.test,
        &body,
        &else_body,
        remaining_stmts,
        cont_label,
        lower_region,
    )
}

pub(crate) fn lower_for_stmt_exit_entries<F>(
    blocks: &mut Vec<BlockPyBlock>,
    else_body: &[Stmt],
    remaining_stmts: &[Stmt],
    cont_label: String,
    lower_region: &mut F,
) -> (String, String)
where
    F: FnMut(&[Stmt], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let rest_entry = lower_region(remaining_stmts, cont_label, None, blocks);
    let exhausted_entry = if else_body.is_empty() {
        rest_entry.clone()
    } else {
        lower_region(else_body, rest_entry.clone(), None, blocks)
    };
    (rest_entry, exhausted_entry)
}

pub(crate) fn lower_for_stmt_body_entry<F>(
    blocks: &mut Vec<BlockPyBlock>,
    loop_continue_label: String,
    body: &[Stmt],
    break_label: String,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Stmt], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let body_entry = lower_region(body, loop_continue_label.clone(), Some(break_label), blocks);
    body_entry
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_for_stmt_sequence<F>(
    for_stmt: ast::StmtFor,
    remaining_stmts: &[Stmt],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    iter_name: &str,
    tmp_name: &str,
    loop_check_label: String,
    loop_continue_label: String,
    assign_label: String,
    setup_label: String,
    assign_body: Vec<Stmt>,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Stmt], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let else_body = suite_ref(&for_stmt.orelse).to_vec();
    let (rest_entry, exhausted_entry) = lower_for_stmt_exit_entries(
        blocks,
        &else_body,
        remaining_stmts,
        cont_label,
        lower_region,
    );

    let body = suite_ref(&for_stmt.body).to_vec();
    let body_entry = lower_for_stmt_body_entry(
        blocks,
        loop_continue_label.clone(),
        &body,
        rest_entry.clone(),
        lower_region,
    );

    emit_for_loop_blocks(
        blocks,
        setup_label,
        assign_label,
        loop_check_label,
        loop_continue_label,
        linear,
        iter_name,
        tmp_name,
        *for_stmt.iter,
        for_stmt.is_async,
        exhausted_entry,
        body_entry,
        assign_body,
    )
}
