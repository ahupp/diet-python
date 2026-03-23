use super::stmt_lowering::lower_stmt_into_with_expr;
use super::*;
use crate::block_py::{BlockPyRaise, BlockPyStmt, BlockPyTerm, Expr};
use crate::passes::annotation_export::build_exec_function_def_binding_stmts;
use crate::passes::ast_to_ast::body::suite_ref;
use crate::passes::ast_to_ast::context::Context;

pub(crate) fn lower_stmts_to_blockpy_stmts_with_context<E>(
    context: &Context,
    stmts: &[Stmt],
) -> Result<crate::block_py::BlockPyCfgFragment<BlockPyStmt<E>, BlockPyTerm<E>>, String>
where
    E: From<Expr> + std::fmt::Debug,
{
    let mut out =
        crate::block_py::BlockPyCfgFragmentBuilder::<BlockPyStmt<E>, BlockPyTerm<E>>::new();
    let mut next_label_id = 0usize;
    for stmt in stmts {
        lower_stmt_into_with_expr(context, stmt, &mut out, None, &mut next_label_id)?;
    }
    Ok(out.finish())
}

pub(crate) fn lower_stmts_to_blockpy_stmts<E>(
    stmts: &[Stmt],
) -> Result<crate::block_py::BlockPyCfgFragment<BlockPyStmt<E>, BlockPyTerm<E>>, String>
where
    E: From<Expr> + std::fmt::Debug,
{
    let context = Context::new(crate::passes::ast_to_ast::Options::for_test(), "");
    lower_stmts_to_blockpy_stmts_with_context(&context, stmts)
}

pub(crate) fn plan_stmt_sequence_head(context: &Context, stmt: &Stmt) -> StmtSequenceHeadPlan {
    super::stmt_lowering::plan_stmt_head_for_blockpy(context, stmt)
}

pub(crate) fn drive_stmt_sequence_until_control(
    context: &Context,
    stmts: &[Stmt],
    mut linear: Vec<Stmt>,
    cell_slots: &HashSet<String>,
    outer_scope_names: &HashSet<String>,
) -> StmtSequenceDriveResult {
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
                linear.extend(build_exec_function_def_binding_stmts(
                    &func_def,
                    cell_slots,
                    outer_scope_names,
                ));
                index += 1;
            }
            StmtSequenceHeadPlan::Delete(delete_stmt) => {
                linear.extend(rewrite_delete_to_deleted_sentinel(&delete_stmt));
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

#[derive(Clone)]
pub(crate) struct LoopLabels {
    pub break_label: String,
    pub continue_label: String,
}

pub(crate) fn lower_common_stmt_sequence_head<FSeq>(
    context: &Context,
    plan: StmtSequenceHeadPlan,
    remaining_stmts: &[Stmt],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    active_exc_target: Option<String>,
    next_label: &mut dyn FnMut() -> String,
    loop_labels: Option<LoopLabels>,
    lower_sequence: &mut FSeq,
) -> Option<String>
where
    FSeq: FnMut(
        &[Stmt],
        String,
        Option<LoopLabels>,
        Option<String>,
        &mut Vec<BlockPyBlock>,
    ) -> String,
{
    match plan {
        StmtSequenceHeadPlan::Raise(raise_stmt) => Some(
            emit_sequence_raise_block_with_expr_setup(
                context,
                blocks,
                next_label(),
                linear,
                compat_blockpy_raise_from_stmt(raise_stmt),
                active_exc_target.as_deref(),
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
                active_exc_target.as_deref(),
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
            active_exc_target.clone(),
            &mut |stmts, cont_label, active_exc_target, blocks| {
                lower_sequence(stmts, cont_label, None, active_exc_target, blocks)
            },
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
                active_exc_target.clone(),
                lower_sequence,
            ))
        }
        StmtSequenceHeadPlan::Break => match loop_labels {
            Some(loop_labels) => Some(emit_sequence_jump_block(
                blocks,
                next_label(),
                linear,
                loop_labels.break_label,
                active_exc_target.as_deref(),
            )),
            None => Some(cont_label),
        },
        StmtSequenceHeadPlan::Continue => match loop_labels {
            Some(loop_labels) => Some(emit_sequence_jump_block(
                blocks,
                next_label(),
                linear,
                loop_labels.continue_label,
                active_exc_target.as_deref(),
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
    active_exc_target: Option<String>,
    iter_name: &str,
    tmp_name: &str,
    loop_check_label: String,
    loop_continue_label: String,
    assign_body: Vec<Stmt>,
    next_block_id: &Cell<usize>,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Stmt], String, Option<LoopLabels>, Option<String>, &mut Vec<BlockPyBlock>) -> String,
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
        active_exc_target,
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
    loop_labels: Option<LoopLabels>,
    active_exc_target: Option<String>,
    blocks: &mut Vec<BlockPyBlock>,
    cell_slots: &HashSet<String>,
    outer_scope_names: &HashSet<String>,
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
        ) {
            StmtSequenceDriveResult::Exhausted { linear } => {
                let label = compat_next_label(fn_name, next_block_id);
                return emit_sequence_jump_block(
                    blocks,
                    label,
                    linear,
                    cont_label,
                    active_exc_target.as_deref(),
                );
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
                    active_exc_target.clone(),
                    &mut || {
                        let mut local_next_id = next_id.get();
                        let label = compat_next_label(fn_name, &mut local_next_id);
                        next_id.set(local_next_id);
                        label
                    },
                    loop_labels.clone(),
                    &mut |stmts, cont_label, nested_loop_labels, active_exc_target, blocks| {
                        if let Some(nested_loop_labels) = nested_loop_labels {
                            let mut local_next_id = next_id.get();
                            let label = lower_stmt_sequence_with_state(
                                context,
                                fn_name,
                                stmts,
                                cont_label.clone(),
                                Some(LoopLabels {
                                    break_label: nested_loop_labels.break_label,
                                    continue_label: cont_label,
                                }),
                                active_exc_target,
                                blocks,
                                cell_slots,
                                outer_scope_names,
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
                                loop_labels.clone(),
                                active_exc_target,
                                blocks,
                                cell_slots,
                                outer_scope_names,
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
                let entry = lower_with_stmt_sequence(
                    fn_name,
                    with_stmt,
                    &stmts[index + 1..],
                    cont_label.clone(),
                    linear,
                    blocks,
                    cell_slots,
                    &next_id,
                    needs_finally_return_flow,
                    active_exc_target.clone(),
                    &mut |stmts, cont_label, active_exc_target, blocks| {
                        let mut local_next_id = next_id.get();
                        let label = lower_stmt_sequence_with_state(
                            context,
                            fn_name,
                            stmts,
                            cont_label,
                            loop_labels.clone(),
                            active_exc_target,
                            blocks,
                            cell_slots,
                            outer_scope_names,
                            &mut local_next_id,
                            next_temp,
                        );
                        next_id.set(local_next_id);
                        label
                    },
                );
                *next_block_id = next_id.into_inner();
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
                    active_exc_target.clone(),
                    iter_name.as_str(),
                    tmp_name.as_str(),
                    loop_check_label,
                    loop_continue_label,
                    assign_body,
                    &next_id,
                    &mut |stmts, cont_label, nested_loop_labels, active_exc_target, blocks| {
                        if let Some(nested_loop_labels) = nested_loop_labels {
                            let mut local_next_id = next_id.get();
                            let label = lower_stmt_sequence_with_state(
                                context,
                                fn_name,
                                stmts,
                                cont_label.clone(),
                                Some(LoopLabels {
                                    break_label: nested_loop_labels.break_label,
                                    continue_label: cont_label,
                                }),
                                active_exc_target,
                                blocks,
                                cell_slots,
                                outer_scope_names,
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
                                loop_labels.clone(),
                                active_exc_target,
                                blocks,
                                cell_slots,
                                outer_scope_names,
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
                        active_exc_target.clone(),
                        &mut |stmts, cont_label, active_exc_target, blocks| {
                            let mut local_next_id = next_id.get();
                            let label = lower_stmt_sequence_with_state(
                                context,
                                fn_name,
                                stmts,
                                cont_label,
                                loop_labels.clone(),
                                active_exc_target,
                                blocks,
                                cell_slots,
                                outer_scope_names,
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
                    let entry = lower_try_stmt_sequence(
                        try_stmt,
                        &stmts[index + 1..],
                        cont_label.clone(),
                        linear,
                        blocks,
                        label.clone(),
                        try_plan,
                        active_exc_target.clone(),
                        &mut |stmts, cont_label, active_exc_target, blocks| {
                            let mut local_next_id = next_id.get();
                            let label = lower_stmt_sequence_with_state(
                                context,
                                fn_name,
                                stmts,
                                cont_label,
                                loop_labels.clone(),
                                active_exc_target,
                                blocks,
                                cell_slots,
                                outer_scope_names,
                                &mut local_next_id,
                                next_temp,
                            );
                            next_id.set(local_next_id);
                            label
                        },
                    );
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
                    active_exc_target.clone(),
                    &mut |stmts, cont_label, active_exc_target, blocks| {
                        lower_stmt_sequence_with_state(
                            context,
                            fn_name,
                            stmts,
                            cont_label,
                            loop_labels.clone(),
                            active_exc_target,
                            blocks,
                            cell_slots,
                            outer_scope_names,
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
    emit_sequence_jump_block(
        blocks,
        label,
        linear,
        cont_label,
        active_exc_target.as_deref(),
    )
}

pub(crate) fn lower_expanded_stmt_sequence<F>(
    desugared_stmts: Vec<Stmt>,
    remaining_stmts: &[Stmt],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    jump_label: Option<String>,
    active_exc_target: Option<String>,
    lower_sequence: &mut F,
) -> String
where
    F: FnMut(&[Stmt], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let mut expanded = desugared_stmts;
    expanded.extend_from_slice(remaining_stmts);
    let expanded_entry = lower_sequence(&expanded, cont_label, active_exc_target.clone(), blocks);
    if linear.is_empty() {
        return expanded_entry;
    }
    let jump_label = jump_label.expect("linear prefix requires a jump label");
    blocks.push(compat_block_from_blockpy(
        jump_label.clone(),
        linear,
        BlockPyTerm::Jump(BlockPyLabel::from(expanded_entry).into()),
    ));
    if let Some(block) = blocks.last_mut() {
        block.exc_edge = active_exc_target
            .as_ref()
            .map(|target| BlockPyEdge::new(BlockPyLabel::from(target.clone())));
    }
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
    active_exc_target: Option<String>,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Stmt], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let then_entry = lower_region(
        then_body,
        rest_entry.clone(),
        active_exc_target.clone(),
        blocks,
    );
    let else_entry = lower_region(else_body, rest_entry, active_exc_target.clone(), blocks);
    emit_if_branch_block_with_expr_setup(
        context,
        blocks,
        label,
        linear,
        test,
        then_entry,
        else_entry,
        active_exc_target.as_deref(),
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
    active_exc_target: Option<String>,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Stmt], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let then_body = suite_ref(&if_stmt.body).to_vec();
    let else_body = extract_if_else_body(&if_stmt);
    let rest_entry = lower_region(
        remaining_stmts,
        cont_label,
        active_exc_target.clone(),
        blocks,
    );
    lower_if_stmt_sequence(
        context,
        blocks,
        label,
        linear,
        *if_stmt.test,
        &then_body,
        &else_body,
        rest_entry,
        active_exc_target,
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
    active_exc_target: Option<String>,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Stmt], String, Option<LoopLabels>, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let rest_entry = lower_region(
        remaining_stmts,
        cont_label,
        None,
        active_exc_target.clone(),
        blocks,
    );
    let cond_false_entry = if else_body.is_empty() {
        rest_entry.clone()
    } else {
        lower_region(
            else_body,
            rest_entry.clone(),
            None,
            active_exc_target.clone(),
            blocks,
        )
    };
    let body_entry = lower_region(
        body,
        test_label.clone(),
        Some(LoopLabels {
            break_label: rest_entry,
            continue_label: test_label.clone(),
        }),
        active_exc_target.clone(),
        blocks,
    );
    emit_simple_while_blocks_with_expr_setup(
        context,
        blocks,
        test_label,
        linear_label,
        linear,
        test,
        body_entry,
        cond_false_entry,
        active_exc_target.as_deref(),
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
    active_exc_target: Option<String>,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Stmt], String, Option<LoopLabels>, Option<String>, &mut Vec<BlockPyBlock>) -> String,
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
        active_exc_target,
        lower_region,
    )
}

pub(crate) fn lower_for_stmt_exit_entries<F>(
    blocks: &mut Vec<BlockPyBlock>,
    else_body: &[Stmt],
    remaining_stmts: &[Stmt],
    cont_label: String,
    active_exc_target: Option<String>,
    lower_region: &mut F,
) -> (String, String)
where
    F: FnMut(&[Stmt], String, Option<LoopLabels>, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let rest_entry = lower_region(
        remaining_stmts,
        cont_label,
        None,
        active_exc_target.clone(),
        blocks,
    );
    let exhausted_entry = if else_body.is_empty() {
        rest_entry.clone()
    } else {
        lower_region(
            else_body,
            rest_entry.clone(),
            None,
            active_exc_target,
            blocks,
        )
    };
    (rest_entry, exhausted_entry)
}

pub(crate) fn lower_for_stmt_body_entry<F>(
    blocks: &mut Vec<BlockPyBlock>,
    loop_continue_label: String,
    body: &[Stmt],
    break_label: String,
    active_exc_target: Option<String>,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Stmt], String, Option<LoopLabels>, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let body_entry = lower_region(
        body,
        loop_continue_label.clone(),
        Some(LoopLabels {
            break_label,
            continue_label: loop_continue_label,
        }),
        active_exc_target,
        blocks,
    );
    body_entry
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_for_stmt_sequence<F>(
    for_stmt: ast::StmtFor,
    remaining_stmts: &[Stmt],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    active_exc_target: Option<String>,
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
    F: FnMut(&[Stmt], String, Option<LoopLabels>, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let else_body = suite_ref(&for_stmt.orelse).to_vec();
    let (rest_entry, exhausted_entry) = lower_for_stmt_exit_entries(
        blocks,
        &else_body,
        remaining_stmts,
        cont_label,
        active_exc_target.clone(),
        lower_region,
    );

    let body = suite_ref(&for_stmt.body).to_vec();
    let body_entry = lower_for_stmt_body_entry(
        blocks,
        loop_continue_label.clone(),
        &body,
        rest_entry.clone(),
        active_exc_target.clone(),
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
        active_exc_target.as_deref(),
    )
}
