use super::stmt_lowering::lower_stmt_into_with_expr;
use super::*;
use crate::basic_block::block_py::{
    BlockPyAssign, BlockPyExpr, BlockPyRaise, BlockPyStmt, BlockPyTerm,
    SemanticBlockPyBlock as BlockPyBlock,
};

pub(crate) fn lower_stmts_to_blockpy_stmts<E>(
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
        lower_stmt_into_with_expr(stmt, &mut out, None, &mut next_label_id)?;
    }
    Ok(out.finish())
}

#[derive(Clone)]
pub(crate) enum GeneratorStmtSequenceHeadKind {
    Stmt(BlockPyStmt),
    Term(BlockPyTerm),
}

fn generator_stmt_sequence_head(stmt: &Stmt) -> Option<(GeneratorStmtSequenceHeadKind, bool)> {
    let generator_stmt =
        match lower_stmts_to_blockpy_stmts::<BlockPyExpr>(std::slice::from_ref(stmt)) {
            Ok(generator_stmt) => generator_stmt,
            Err(err) => {
                return match stmt {
                    Stmt::Expr(_) | Stmt::Assign(_) | Stmt::Return(_) => {
                        panic!("failed to convert generator stmt to BlockPy before lowering: {err}")
                    }
                    _ => None,
                };
            }
        };
    let generator_stmt = match (generator_stmt.body.as_slice(), generator_stmt.term.as_ref()) {
        ([stmt], None) => GeneratorStmtSequenceHeadKind::Stmt(stmt.clone()),
        ([], Some(term)) => GeneratorStmtSequenceHeadKind::Term(term.clone()),
        _ => panic!("generator stmt conversion should yield one BlockPy stmt or one term"),
    };
    match &generator_stmt {
        GeneratorStmtSequenceHeadKind::Stmt(BlockPyStmt::Expr(BlockPyExpr::Yield(_)))
        | GeneratorStmtSequenceHeadKind::Stmt(BlockPyStmt::Expr(BlockPyExpr::YieldFrom(_)))
        | GeneratorStmtSequenceHeadKind::Stmt(BlockPyStmt::Assign(BlockPyAssign {
            value: BlockPyExpr::Yield(_),
            ..
        }))
        | GeneratorStmtSequenceHeadKind::Stmt(BlockPyStmt::Assign(BlockPyAssign {
            value: BlockPyExpr::YieldFrom(_),
            ..
        }))
        | GeneratorStmtSequenceHeadKind::Term(BlockPyTerm::Return(Some(BlockPyExpr::Yield(_))))
        | GeneratorStmtSequenceHeadKind::Term(BlockPyTerm::Return(Some(BlockPyExpr::YieldFrom(
            _,
        )))) => {}
        _ => return None,
    }
    let needs_rest_entry = match &generator_stmt {
        GeneratorStmtSequenceHeadKind::Stmt(stmt) => {
            blockpy_stmt_requires_generator_rest_entry(stmt)
        }
        GeneratorStmtSequenceHeadKind::Term(_) => false,
    };
    Some((generator_stmt, needs_rest_entry))
}

#[derive(Clone)]
pub(crate) struct GeneratorStmtSequencePlan {
    generator_head: GeneratorStmtSequenceHeadKind,
    pub needs_rest_entry: bool,
}

pub(crate) fn plan_generator_stmt_in_sequence(stmt: &Stmt) -> Option<GeneratorStmtSequencePlan> {
    let (generator_head, needs_rest_entry) = generator_stmt_sequence_head(stmt)?;
    Some(GeneratorStmtSequencePlan {
        generator_head,
        needs_rest_entry,
    })
}

pub(crate) fn plan_stmt_sequence_head(
    stmt: &Stmt,
    allow_generator_head: bool,
) -> StmtSequenceHeadPlan {
    if allow_generator_head {
        match stmt {
            Stmt::Expr(_) | Stmt::Assign(_) | Stmt::Return(_) => {
                if let Some(plan) = plan_generator_stmt_in_sequence(stmt) {
                    return StmtSequenceHeadPlan::Generator {
                        plan,
                        sync_target_cells: matches!(stmt, Stmt::Assign(_)),
                    };
                }
            }
            _ => {}
        }
    }

    match stmt {
        Stmt::Expr(_) | Stmt::Pass(_) | Stmt::Assign(_) | Stmt::Global(_) | Stmt::Nonlocal(_) => {
            StmtSequenceHeadPlan::Linear(stmt.clone())
        }
        Stmt::FunctionDef(func_def) => StmtSequenceHeadPlan::FunctionDef(func_def.clone()),
        Stmt::Raise(raise_stmt) => StmtSequenceHeadPlan::Raise(raise_stmt.clone()),
        Stmt::Delete(delete_stmt) => StmtSequenceHeadPlan::Delete(delete_stmt.clone()),
        Stmt::Return(ret) => {
            StmtSequenceHeadPlan::Return(ret.value.as_ref().map(|expr| *expr.clone()))
        }
        Stmt::If(if_stmt) => StmtSequenceHeadPlan::If(if_stmt.clone()),
        Stmt::While(while_stmt) => StmtSequenceHeadPlan::While(while_stmt.clone()),
        Stmt::For(for_stmt) => StmtSequenceHeadPlan::For(for_stmt.clone()),
        Stmt::Try(try_stmt) => StmtSequenceHeadPlan::Try(try_stmt.clone()),
        Stmt::With(with_stmt) => StmtSequenceHeadPlan::With(with_stmt.clone()),
        Stmt::Break(_) => StmtSequenceHeadPlan::Break,
        Stmt::Continue(_) => StmtSequenceHeadPlan::Continue,
        _ => StmtSequenceHeadPlan::Unsupported,
    }
}

pub(crate) fn drive_stmt_sequence_until_control<FDef, FDelete>(
    stmts: &[Box<Stmt>],
    mut linear: Vec<Stmt>,
    allow_generator_head: bool,
    lower_non_bb_def: &mut FDef,
    rewrite_delete: &mut FDelete,
) -> StmtSequenceDriveResult
where
    FDef: FnMut(&ast::StmtFunctionDef) -> Vec<Stmt>,
    FDelete: FnMut(&ast::StmtDelete) -> Vec<Stmt>,
{
    let mut index = 0;
    while index < stmts.len() {
        match plan_stmt_sequence_head(stmts[index].as_ref(), allow_generator_head) {
            StmtSequenceHeadPlan::Linear(stmt) => {
                linear.push(stmt);
                index += 1;
            }
            StmtSequenceHeadPlan::FunctionDef(func_def) => {
                if func_def.name.id.as_str().starts_with("_dp_bb_") {
                    linear.push(Stmt::FunctionDef(func_def));
                } else {
                    linear.extend(lower_non_bb_def(&func_def));
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
                }
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
    plan: StmtSequenceHeadPlan,
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    next_label: &mut dyn FnMut() -> String,
    break_label: Option<String>,
    continue_label: Option<String>,
    lower_sequence: &mut FSeq,
) -> Option<String>
where
    FSeq: FnMut(&[Box<Stmt>], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    match plan {
        StmtSequenceHeadPlan::Raise(raise_stmt) => Some(emit_sequence_raise_block(
            blocks,
            next_label(),
            linear,
            compat_blockpy_raise_from_stmt(raise_stmt),
        )),
        StmtSequenceHeadPlan::Return(value) => Some(emit_sequence_return_block(
            blocks,
            next_label(),
            linear,
            value,
        )),
        StmtSequenceHeadPlan::If(if_stmt) => Some(lower_if_stmt_sequence_from_stmt(
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
    remaining_stmts: &[Box<Stmt>],
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
    F: FnMut(&[Box<Stmt>], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
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

pub(crate) fn lower_generator_stmt_sequence_plan(
    plan: &GeneratorStmtSequencePlan,
    linear: Vec<Stmt>,
    rest_entry: Option<String>,
    blocks: &mut Vec<BlockPyBlock>,
    closure_state: bool,
    try_regions: &mut Vec<TryRegionPlan>,
    resume_order: &mut Vec<String>,
    yield_sites: &mut Vec<GeneratorYieldSite>,
    next_block_id: &mut usize,
    fn_name: &str,
    cell_slots: Option<&std::collections::HashSet<String>>,
) -> Option<String> {
    match &plan.generator_head {
        GeneratorStmtSequenceHeadKind::Stmt(stmt) => lower_generator_blockpy_stmt_in_sequence(
            stmt,
            linear,
            rest_entry,
            blocks,
            None,
            closure_state,
            try_regions,
            resume_order,
            yield_sites,
            next_block_id,
            fn_name,
            cell_slots,
        ),
        GeneratorStmtSequenceHeadKind::Term(term) => lower_generator_blockpy_term_in_sequence(
            term,
            linear,
            blocks,
            None,
            closure_state,
            try_regions,
            resume_order,
            yield_sites,
            next_block_id,
            fn_name,
        ),
    }
}

pub(crate) fn lower_generator_stmt_sequence_head<F>(
    fn_name: &str,
    plan: GeneratorStmtSequencePlan,
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    mut state: GeneratorStmtSequenceLoweringState,
    try_regions: &mut Vec<TryRegionPlan>,
    cell_slots: Option<&std::collections::HashSet<String>>,
    lower_rest: &mut F,
) -> (Option<String>, GeneratorStmtSequenceLoweringState)
where
    F: FnMut(
        &[Box<Stmt>],
        String,
        &mut Vec<BlockPyBlock>,
        GeneratorStmtSequenceLoweringState,
    ) -> (String, GeneratorStmtSequenceLoweringState),
{
    let rest_entry = if plan.needs_rest_entry {
        let (entry, updated_state) = lower_rest(remaining_stmts, cont_label, blocks, state);
        state = updated_state;
        Some(entry)
    } else {
        None
    };
    let label = lower_generator_stmt_sequence_plan(
        &plan,
        linear,
        rest_entry,
        blocks,
        state.closure_state,
        try_regions,
        &mut state.resume_order,
        &mut state.yield_sites,
        &mut state.next_block_id,
        fn_name,
        cell_slots,
    );
    (label, state)
}

pub(crate) fn lower_stmt_sequence_with_state<FDef, FTemp>(
    fn_name: &str,
    stmts: &[Box<Stmt>],
    cont_label: String,
    break_label: Option<String>,
    continue_label: Option<String>,
    blocks: &mut Vec<BlockPyBlock>,
    cell_slots: &HashSet<String>,
    generator_state: &mut BlockPySequenceGeneratorState,
    try_regions: &mut Vec<TryRegionPlan>,
    next_block_id: &mut usize,
    lower_non_bb_def: &mut FDef,
    next_temp: &mut FTemp,
) -> String
where
    FDef: FnMut(&ast::StmtFunctionDef) -> Vec<Stmt>,
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
            &stmts[index..],
            linear,
            generator_state.enabled,
            lower_non_bb_def,
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
            StmtSequenceHeadPlan::Generator {
                plan,
                sync_target_cells,
            } => {
                let initial_state = GeneratorStmtSequenceLoweringState {
                    enabled: generator_state.enabled,
                    closure_state: generator_state.closure_state,
                    resume_order: std::mem::take(&mut generator_state.resume_order),
                    yield_sites: std::mem::take(&mut generator_state.yield_sites),
                    next_block_id: *next_block_id,
                };
                let mut local_try_regions = Vec::new();
                let (label, updated_state) = lower_generator_stmt_sequence_head(
                    fn_name,
                    plan,
                    &stmts[index + 1..],
                    cont_label.clone(),
                    linear.clone(),
                    blocks,
                    initial_state,
                    &mut local_try_regions,
                    sync_target_cells.then_some(cell_slots),
                    &mut |stmts, cont_label, blocks, state| {
                        let mut local_generator_state = BlockPySequenceGeneratorState {
                            enabled: state.enabled,
                            closure_state: state.closure_state,
                            resume_order: state.resume_order,
                            yield_sites: state.yield_sites,
                        };
                        let mut local_next_block_id = state.next_block_id;
                        let label = lower_stmt_sequence_with_state(
                            fn_name,
                            stmts,
                            cont_label,
                            break_label.clone(),
                            continue_label.clone(),
                            blocks,
                            cell_slots,
                            &mut local_generator_state,
                            try_regions,
                            &mut local_next_block_id,
                            lower_non_bb_def,
                            next_temp,
                        );
                        (
                            label,
                            GeneratorStmtSequenceLoweringState {
                                enabled: local_generator_state.enabled,
                                closure_state: local_generator_state.closure_state,
                                resume_order: local_generator_state.resume_order,
                                yield_sites: local_generator_state.yield_sites,
                                next_block_id: local_next_block_id,
                            },
                        )
                    },
                );
                try_regions.extend(local_try_regions);
                *next_block_id = updated_state.next_block_id;
                generator_state.resume_order = updated_state.resume_order;
                generator_state.yield_sites = updated_state.yield_sites;
                if let Some(label) = label {
                    return label;
                }
                linear.push(stmts[index].as_ref().clone());
                index += 1;
                continue;
            }
            plan @ (StmtSequenceHeadPlan::Raise(_)
            | StmtSequenceHeadPlan::Return(_)
            | StmtSequenceHeadPlan::If(_)
            | StmtSequenceHeadPlan::While(_)
            | StmtSequenceHeadPlan::Break
            | StmtSequenceHeadPlan::Continue) => {
                let next_id = Cell::new(*next_block_id);
                let label = lower_common_stmt_sequence_head(
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
                                fn_name,
                                stmts,
                                cont_label.clone(),
                                Some(loop_break_label),
                                Some(cont_label),
                                blocks,
                                cell_slots,
                                generator_state,
                                try_regions,
                                &mut local_next_id,
                                lower_non_bb_def,
                                next_temp,
                            );
                            next_id.set(local_next_id);
                            label
                        } else {
                            let mut local_next_id = next_id.get();
                            let label = lower_stmt_sequence_with_state(
                                fn_name,
                                stmts,
                                cont_label,
                                break_label.clone(),
                                continue_label.clone(),
                                blocks,
                                cell_slots,
                                generator_state,
                                try_regions,
                                &mut local_next_id,
                                lower_non_bb_def,
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
                let needs_finally_return_flow = contains_return_stmt_in_body(&with_stmt.body.body);
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
                            fn_name,
                            stmts,
                            cont_label,
                            break_label.clone(),
                            continue_label.clone(),
                            blocks,
                            cell_slots,
                            generator_state,
                            try_regions,
                            &mut local_next_id,
                            lower_non_bb_def,
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
                let continue_state = GeneratorStmtSequenceLoweringState {
                    enabled: generator_state.enabled,
                    closure_state: generator_state.closure_state,
                    resume_order: std::mem::take(&mut generator_state.resume_order),
                    yield_sites: std::mem::take(&mut generator_state.yield_sites),
                    next_block_id: *next_block_id,
                };
                let (loop_continue_label, continue_state) =
                    lower_for_loop_continue_entry_with_state(
                        blocks,
                        fn_name,
                        iter_name.as_str(),
                        tmp_name.as_str(),
                        loop_check_label.clone(),
                        for_stmt.is_async,
                        try_regions,
                        continue_state,
                    );
                *next_block_id = continue_state.next_block_id;
                generator_state.resume_order = continue_state.resume_order;
                generator_state.yield_sites = continue_state.yield_sites;
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
                                fn_name,
                                stmts,
                                cont_label.clone(),
                                Some(loop_break_label),
                                Some(cont_label),
                                blocks,
                                cell_slots,
                                generator_state,
                                try_regions,
                                &mut local_next_id,
                                lower_non_bb_def,
                                next_temp,
                            );
                            next_id.set(local_next_id);
                            label
                        } else {
                            let mut local_next_id = next_id.get();
                            let label = lower_stmt_sequence_with_state(
                                fn_name,
                                stmts,
                                cont_label,
                                break_label.clone(),
                                continue_label.clone(),
                                blocks,
                                cell_slots,
                                generator_state,
                                try_regions,
                                &mut local_next_id,
                                lower_non_bb_def,
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
                                fn_name,
                                stmts,
                                cont_label,
                                break_label.clone(),
                                continue_label.clone(),
                                blocks,
                                cell_slots,
                                generator_state,
                                try_regions,
                                &mut local_next_id,
                                lower_non_bb_def,
                                next_temp,
                            );
                            next_id.set(local_next_id);
                            label
                        },
                    )
                } else {
                    let mut local_next_id = next_id.get();
                    let has_finally = !try_stmt.finalbody.body.is_empty();
                    let needs_finally_return_flow = has_finally
                        && (contains_return_stmt_in_body(&try_stmt.body.body)
                            || contains_return_stmt_in_handlers(&try_stmt.handlers)
                            || contains_return_stmt_in_body(&try_stmt.orelse.body));
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
                                fn_name,
                                stmts,
                                cont_label,
                                break_label.clone(),
                                continue_label.clone(),
                                blocks,
                                cell_slots,
                                generator_state,
                                try_regions,
                                &mut local_next_id,
                                lower_non_bb_def,
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
            StmtSequenceHeadPlan::Unsupported => return cont_label,
        }
    }

    let label = compat_next_label(fn_name, next_block_id);
    emit_sequence_jump_block(blocks, label, linear, cont_label)
}

pub(crate) fn lower_expanded_stmt_sequence<F>(
    desugared_stmt: Stmt,
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
    let mut expanded = match desugared_stmt {
        Stmt::BodyStmt(body) => body.body,
        stmt => vec![Box::new(stmt)],
    };
    expanded.extend(remaining_stmts.iter().cloned());
    let expanded_entry = lower_sequence(&expanded, cont_label, blocks);
    if linear.is_empty() {
        return expanded_entry;
    }
    let jump_label = jump_label.expect("linear prefix requires a jump label");
    blocks.push(compat_block_from_blockpy(
        jump_label.clone(),
        linear,
        BlockPyTerm::Jump(BlockPyLabel::from(expanded_entry)),
    ));
    jump_label
}

pub(crate) fn lower_if_stmt_sequence<F>(
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    linear: Vec<Stmt>,
    test: Expr,
    then_body: &[Box<Stmt>],
    else_body: &[Box<Stmt>],
    rest_entry: String,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Box<Stmt>], String, &mut Vec<BlockPyBlock>) -> String,
{
    let then_entry = lower_region(then_body, rest_entry.clone(), blocks);
    let else_entry = lower_region(else_body, rest_entry, blocks);
    emit_if_branch_block(blocks, label, linear, test, then_entry, else_entry)
}

pub(crate) fn lower_if_stmt_sequence_from_stmt<F>(
    if_stmt: ast::StmtIf,
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Box<Stmt>], String, &mut Vec<BlockPyBlock>) -> String,
{
    let then_body = flatten_stmt_boxes(&if_stmt.body.body);
    let else_body = flatten_stmt_boxes(&extract_if_else_body(&if_stmt));
    let rest_entry = lower_region(remaining_stmts, cont_label, blocks);
    lower_if_stmt_sequence(
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

fn extract_if_else_body(if_stmt: &ast::StmtIf) -> Vec<Box<Stmt>> {
    if if_stmt.elif_else_clauses.is_empty() {
        return Vec::new();
    }
    if_stmt
        .elif_else_clauses
        .first()
        .map(|clause| clause.body.body.clone())
        .unwrap_or_default()
}

pub(crate) fn lower_while_stmt_sequence<F>(
    blocks: &mut Vec<BlockPyBlock>,
    test_label: String,
    linear_label: Option<String>,
    linear: Vec<Stmt>,
    test: Expr,
    body: &[Box<Stmt>],
    else_body: &[Box<Stmt>],
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Box<Stmt>], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let rest_entry = lower_region(remaining_stmts, cont_label, None, blocks);
    let cond_false_entry = if else_body.is_empty() {
        rest_entry.clone()
    } else {
        lower_region(else_body, rest_entry.clone(), None, blocks)
    };
    let body_entry = lower_region(body, test_label.clone(), Some(rest_entry), blocks);
    emit_simple_while_blocks(
        blocks,
        test_label,
        linear_label,
        linear,
        test,
        body_entry,
        cond_false_entry,
    )
}

pub(crate) fn lower_while_stmt_sequence_from_stmt<F>(
    while_stmt: ast::StmtWhile,
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    test_label: String,
    linear_label: Option<String>,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Box<Stmt>], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let body = flatten_stmt_boxes(&while_stmt.body.body);
    let else_body = flatten_stmt_boxes(&while_stmt.orelse.body);
    lower_while_stmt_sequence(
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
    else_body: &[Box<Stmt>],
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    lower_region: &mut F,
) -> (String, String)
where
    F: FnMut(&[Box<Stmt>], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
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
    body: &[Box<Stmt>],
    break_label: String,
    lower_region: &mut F,
) -> String
where
    F: FnMut(&[Box<Stmt>], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let body_entry = lower_region(body, loop_continue_label.clone(), Some(break_label), blocks);
    body_entry
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_for_stmt_sequence<F>(
    for_stmt: ast::StmtFor,
    remaining_stmts: &[Box<Stmt>],
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
    F: FnMut(&[Box<Stmt>], String, Option<String>, &mut Vec<BlockPyBlock>) -> String,
{
    let else_body = flatten_stmt_boxes(&for_stmt.orelse.body);
    let (rest_entry, exhausted_entry) = lower_for_stmt_exit_entries(
        blocks,
        &else_body,
        remaining_stmts,
        cont_label,
        lower_region,
    );

    let body = flatten_stmt_boxes(&for_stmt.body.body);
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
