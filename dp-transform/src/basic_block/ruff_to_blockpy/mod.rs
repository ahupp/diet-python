use super::ast_to_bb::{
    compute_exception_edge_by_label_blockpy, contains_return_stmt_in_body,
    contains_return_stmt_in_handlers, flatten_stmt_boxes, fold_constant_brif_blockpy,
    fold_jumps_to_trivial_none_return_blockpy, prune_unreachable_blockpy_blocks,
    relabel_blockpy_blocks, rewrite_exception_accesses_shared, sync_target_cells_stmts_shared,
    rewrite_region_returns_to_finally_blockpy_shared, FunctionIdentityByNode,
};
use super::await_lower::{
    coroutine_generator_marker_stmt, lower_coroutine_awaits_in_stmt,
    lower_coroutine_awaits_in_stmts, lower_coroutine_awaits_to_yield_from,
};
use super::bb_ir::BindingTarget;
use super::block_py::{
    BlockPyAssign, BlockPyBlock, BlockPyDelete, BlockPyExceptHandler, BlockPyExceptHandlerKind,
    BlockPyFunction, BlockPyFunctionKind, BlockPyIf, BlockPyLabel, BlockPyLegacyTryJump,
    BlockPyModule, BlockPyRaise, BlockPyStmt, BlockPyTry,
};
use crate::namegen::fresh_name;
use crate::ruff_ast_to_string;
use crate::template::{empty_body, into_body, is_simple};
use crate::transform::ast_rewrite::Rewrite;
use crate::transform::rewrite_expr::make_tuple;
use crate::transform::rewrite_stmt;
use crate::transformer::walk_stmt;
use crate::transformer::{walk_expr, Transformer};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt, StmtBody};
use std::cell::Cell;
use std::collections::HashMap;

mod generator_lowering;

pub(crate) use generator_lowering::{
    blockpy_stmt_requires_generator_rest_entry, build_async_for_continue_entry,
    build_closure_backed_generator_factory_block, build_initial_generator_metadata,
    lower_generator_blockpy_blocks, lower_generator_blockpy_stmt_in_sequence,
    lower_generator_yield_terms_to_explicit_return_blockpy, synthesize_generator_dispatch_metadata,
    BlockPyGeneratorLoweringResult, GeneratorYieldSite,
};

pub(crate) struct BlockPySequenceGeneratorState {
    pub closure_state: bool,
    pub resume_order: Vec<String>,
    pub yield_sites: Vec<GeneratorYieldSite>,
}

pub(crate) struct GeneratorStmtSequenceLoweringState {
    pub closure_state: bool,
    pub resume_order: Vec<String>,
    pub yield_sites: Vec<GeneratorYieldSite>,
    pub next_block_id: usize,
}

#[derive(Clone)]
pub(crate) enum StmtSequenceHeadPlan {
    Linear(Stmt),
    FunctionDef(ast::StmtFunctionDef),
    Generator {
        plan: GeneratorStmtSequencePlan,
        sync_target_cells: bool,
    },
    Raise(ast::StmtRaise),
    Delete(ast::StmtDelete),
    Return(Option<Expr>),
    If(ast::StmtIf),
    While(ast::StmtWhile),
    For(ast::StmtFor),
    Try(ast::StmtTry),
    With(ast::StmtWith),
    Break,
    Continue,
    Unsupported,
}

pub(crate) enum StmtSequenceDriveResult {
    Exhausted {
        linear: Vec<Stmt>,
    },
    Break {
        linear: Vec<Stmt>,
        index: usize,
        plan: StmtSequenceHeadPlan,
    },
}

pub(crate) fn blockpy_kind_for_lowered_runtime(
    is_async_generator_runtime: bool,
    coroutine_via_generator: bool,
    has_yield: bool,
) -> BlockPyFunctionKind {
    if is_async_generator_runtime {
        BlockPyFunctionKind::AsyncGenerator
    } else if has_yield {
        BlockPyFunctionKind::Generator
    } else if coroutine_via_generator {
        BlockPyFunctionKind::Coroutine
    } else {
        BlockPyFunctionKind::Function
    }
}

pub(crate) fn build_blockpy_function(
    bind_name: String,
    qualname: String,
    binding_target: BindingTarget,
    kind: BlockPyFunctionKind,
    params: ast::Parameters,
    blocks: Vec<BlockPyBlock>,
    generator: Option<super::block_py::BlockPyGeneratorInfo>,
) -> BlockPyFunction {
    BlockPyFunction {
        bind_name,
        qualname,
        binding_target,
        kind,
        generator,
        params,
        blocks,
    }
}

pub(crate) struct PendingBlockPyGeneratorInfo {
    pub closure_state: bool,
    pub resume_order: Vec<String>,
    pub yield_sites: Vec<GeneratorYieldSite>,
}

pub(crate) fn build_finalized_blockpy_function(
    bind_name: String,
    qualname: String,
    binding_target: BindingTarget,
    kind: BlockPyFunctionKind,
    params: ast::Parameters,
    blocks: Vec<BlockPyBlock>,
    entry_label: String,
    end_label: String,
    label_prefix: &str,
    generator: Option<PendingBlockPyGeneratorInfo>,
    is_async_generator_runtime: bool,
    is_closure_backed_generator_runtime: bool,
    uncaught_exc_name: String,
) -> (BlockPyFunction, String) {
    let function = build_blockpy_function(
        bind_name,
        qualname,
        binding_target,
        kind,
        params,
        blocks,
        generator.map(|generator| {
            build_initial_generator_metadata(
                entry_label.as_str(),
                generator.closure_state,
                &generator.resume_order,
                &generator.yield_sites,
            )
        }),
    );
    finalize_blockpy_function(
        function,
        entry_label,
        end_label,
        label_prefix,
        is_async_generator_runtime,
        is_closure_backed_generator_runtime,
        uncaught_exc_name,
    )
}

pub(crate) fn compute_blockpy_exception_edges(
    function: &BlockPyFunction,
) -> HashMap<String, (Option<String>, Option<String>)> {
    let mut exception_edges = compute_exception_edge_by_label_blockpy(&function.blocks);
    if let Some(generator_info) = function.generator.as_ref() {
        if let (Some(uncaught_label), Some(uncaught_exc_name)) = (
            generator_info.uncaught_block_label.as_ref(),
            generator_info.uncaught_exc_name.as_ref(),
        ) {
            for block in &function.blocks {
                let label = block.label.as_str();
                if generator_info
                    .done_block_label
                    .as_ref()
                    .map(|done| done.as_str() == label)
                    .unwrap_or(false)
                    || generator_info
                        .invalid_block_label
                        .as_ref()
                        .map(|invalid| invalid.as_str() == label)
                        .unwrap_or(false)
                    || Some(label)
                        == generator_info
                            .uncaught_block_label
                            .as_ref()
                            .map(BlockPyLabel::as_str)
                    || generator_info
                        .throw_passthrough_labels
                        .iter()
                        .any(|passthrough| passthrough.as_str() == label)
                {
                    continue;
                }
                exception_edges.entry(label.to_string()).or_insert((
                    Some(uncaught_label.as_str().to_string()),
                    Some(uncaught_exc_name.clone()),
                ));
            }
        }
    }
    exception_edges
}

pub(crate) fn compat_block_from_blockpy(
    label: String,
    body: Vec<Stmt>,
    terminal: BlockPyStmt,
) -> BlockPyBlock {
    let mut body = lower_stmts_to_blockpy_stmts(&body).unwrap_or_else(|err| {
        panic!("failed to convert compatibility block body to BlockPy: {err}")
    });
    body.push(terminal);
    BlockPyBlock {
        label: BlockPyLabel::from(label),
        body,
    }
}

pub(crate) fn compat_if_jump_block(
    label: String,
    body: Vec<Stmt>,
    test: Expr,
    then_label: String,
    else_label: String,
) -> BlockPyBlock {
    compat_block_from_blockpy(
        label.clone(),
        body,
        BlockPyStmt::If(BlockPyIf {
            test: test.into(),
            body: vec![BlockPyBlock {
                label: BlockPyLabel::from(format!("{label}_if_true")),
                body: vec![BlockPyStmt::Jump(BlockPyLabel::from(then_label))],
            }],
            orelse: vec![BlockPyBlock {
                label: BlockPyLabel::from(format!("{label}_if_false")),
                body: vec![BlockPyStmt::Jump(BlockPyLabel::from(else_label))],
            }],
        }),
    )
}

pub(crate) fn compat_jump_block_from_blockpy(
    label: String,
    body: Vec<Stmt>,
    target_label: String,
) -> BlockPyBlock {
    compat_block_from_blockpy(
        label,
        body,
        BlockPyStmt::Jump(BlockPyLabel::from(target_label)),
    )
}

pub(crate) fn compat_return_block_from_expr(
    label: String,
    body: Vec<Stmt>,
    value: Option<Expr>,
) -> BlockPyBlock {
    compat_block_from_blockpy(label, body, BlockPyStmt::Return(value.map(Into::into)))
}

pub(crate) fn compat_raise_block_from_blockpy_raise(
    label: String,
    body: Vec<Stmt>,
    exc: BlockPyRaise,
) -> BlockPyBlock {
    compat_block_from_blockpy(label, body, BlockPyStmt::Raise(exc))
}

pub(crate) fn emit_sequence_jump_block(
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    linear: Vec<Stmt>,
    target_label: String,
) -> String {
    blocks.push(compat_jump_block_from_blockpy(
        label.clone(),
        linear,
        target_label,
    ));
    label
}

pub(crate) fn emit_sequence_return_block(
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    linear: Vec<Stmt>,
    value: Option<Expr>,
) -> String {
    blocks.push(compat_return_block_from_expr(label.clone(), linear, value));
    label
}

pub(crate) fn emit_sequence_raise_block(
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    linear: Vec<Stmt>,
    exc: BlockPyRaise,
) -> String {
    blocks.push(compat_raise_block_from_blockpy_raise(
        label.clone(),
        linear,
        exc,
    ));
    label
}

pub(crate) fn emit_if_branch_block(
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    body: Vec<Stmt>,
    test: Expr,
    then_label: String,
    else_label: String,
) -> String {
    blocks.push(compat_if_jump_block(
        label.clone(),
        body,
        test,
        then_label,
        else_label,
    ));
    label
}

pub(crate) fn emit_simple_while_blocks(
    blocks: &mut Vec<BlockPyBlock>,
    test_label: String,
    linear_label: Option<String>,
    linear: Vec<Stmt>,
    test: Expr,
    body_entry: String,
    cond_false_entry: String,
) -> String {
    blocks.push(compat_if_jump_block(
        test_label.clone(),
        Vec::new(),
        test,
        body_entry,
        cond_false_entry,
    ));
    if let Some(linear_label) = linear_label {
        blocks.push(compat_jump_block_from_blockpy(
            linear_label.clone(),
            linear,
            test_label,
        ));
        linear_label
    } else {
        test_label
    }
}

pub(crate) fn emit_for_loop_blocks(
    blocks: &mut Vec<BlockPyBlock>,
    setup_label: String,
    assign_label: String,
    loop_check_label: String,
    loop_continue_label: String,
    linear: Vec<Stmt>,
    iter_name: &str,
    tmp_name: &str,
    iterable: Expr,
    is_async: bool,
    exhausted_entry: String,
    body_entry: String,
    assign_body: Vec<Stmt>,
) -> String {
    let iter_expr = py_expr!("{iter:id}", iter = iter_name);
    let tmp_expr = py_expr!("{tmp:id}", tmp = tmp_name);

    blocks.push(compat_block_from_blockpy(
        assign_label.clone(),
        assign_body,
        BlockPyStmt::Jump(BlockPyLabel::from(body_entry)),
    ));

    let exhausted_test = py_expr!(
        "__dp_is_({value:expr}, __dp__.ITER_COMPLETE)",
        value = tmp_expr
    );
    let check_body = if is_async {
        Vec::new()
    } else {
        vec![py_stmt!(
            "{tmp:id} = __dp_next_or_sentinel({iter:expr})",
            tmp = tmp_name,
            iter = iter_expr.clone(),
        )]
    };
    blocks.push(compat_if_jump_block(
        loop_check_label.clone(),
        check_body,
        exhausted_test,
        exhausted_entry,
        assign_label,
    ));

    let mut setup_body = linear;
    if is_async {
        setup_body.push(py_stmt!(
            "{iter:id} = __dp_aiter({iterable:expr})",
            iter = iter_name,
            iterable = iterable,
        ));
    } else {
        setup_body.push(py_stmt!(
            "{iter:id} = __dp_iter({iterable:expr})",
            iter = iter_name,
            iterable = iterable,
        ));
    }
    blocks.push(compat_block_from_blockpy(
        setup_label.clone(),
        setup_body,
        BlockPyStmt::Jump(BlockPyLabel::from(loop_continue_label)),
    ));
    setup_label
}

pub(crate) fn lower_for_loop_continue_entry_with_state(
    blocks: &mut Vec<BlockPyBlock>,
    fn_name: &str,
    iter_name: &str,
    tmp_name: &str,
    loop_check_label: String,
    is_async: bool,
    mut state: GeneratorStmtSequenceLoweringState,
) -> (String, GeneratorStmtSequenceLoweringState) {
    let entry = if is_async {
        build_async_for_continue_entry(
            blocks,
            fn_name,
            py_expr!("{iter:id}", iter = iter_name),
            tmp_name,
            loop_check_label.as_str(),
            state.closure_state,
            &mut state.resume_order,
            &mut state.yield_sites,
            &mut state.next_block_id,
        )
    } else {
        loop_check_label
    };
    (entry, state)
}

fn compat_next_temp(prefix: &str, next_id: &mut usize) -> String {
    let current = *next_id;
    *next_id += 1;
    format!("_dp_{prefix}_{current}")
}

fn compat_sanitize_ident(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn compat_next_label(fn_name: &str, next_id: &mut usize) -> String {
    let current = *next_id;
    *next_id += 1;
    format!("_dp_bb_{}_{}", compat_sanitize_ident(fn_name), current)
}

#[derive(Debug, Clone)]
pub(crate) struct LegacyTryPlan {
    pub finally_reason_name: Option<String>,
    pub finally_return_value_name: Option<String>,
    pub finally_dispatch_label: Option<String>,
    pub finally_return_label: Option<String>,
    pub finally_exc_name: Option<String>,
    pub body_pass_label: String,
    pub except_pass_label: String,
    pub except_exc_name: String,
}

pub(crate) fn build_legacy_try_plan(
    fn_name: &str,
    has_finally: bool,
    needs_finally_return_flow: bool,
    next_id: &mut usize,
) -> LegacyTryPlan {
    let finally_reason_name = if has_finally && needs_finally_return_flow {
        Some(compat_next_temp("try_reason", next_id))
    } else {
        None
    };
    let finally_return_value_name = if has_finally && needs_finally_return_flow {
        Some(compat_next_temp("try_value", next_id))
    } else {
        None
    };
    let finally_dispatch_label = if has_finally && needs_finally_return_flow {
        Some(compat_next_label(fn_name, next_id))
    } else {
        None
    };
    let finally_return_label = if has_finally && needs_finally_return_flow {
        Some(compat_next_label(fn_name, next_id))
    } else {
        None
    };
    let finally_exc_name = if has_finally {
        Some(compat_next_temp("try_exc", next_id))
    } else {
        None
    };

    LegacyTryPlan {
        finally_reason_name,
        finally_return_value_name,
        finally_dispatch_label,
        finally_return_label,
        finally_exc_name,
        body_pass_label: compat_next_label(fn_name, next_id),
        except_pass_label: compat_next_label(fn_name, next_id),
        except_exc_name: compat_next_temp("try_exc", next_id),
    }
}

impl LegacyTryPlan {
    pub(crate) fn finally_cont_label(&self, rest_entry: &str) -> String {
        self.finally_dispatch_label
            .clone()
            .unwrap_or_else(|| rest_entry.to_string())
    }

    pub(crate) fn finally_fallthrough_label(&self, rest_entry: &str) -> Option<String> {
        self.finally_dispatch_label
            .clone()
            .or_else(|| Some(rest_entry.to_string()))
    }

    pub(crate) fn pass_target(&self, finally_label: Option<&str>, rest_entry: &str) -> String {
        finally_label
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| rest_entry.to_string())
    }
}

pub(crate) fn prepare_finally_body(
    finalbody: &ast::StmtBody,
    finally_exc_name: &str,
) -> Vec<Box<Stmt>> {
    let mut finally_body = flatten_stmt_boxes(&finalbody.body);
    finally_body = rewrite_exception_accesses_shared(finally_body, finally_exc_name);
    finally_body.push(Box::new(py_stmt!(
        "if __dp_is_not({exc:id}, None):\n    raise {exc:id}",
        exc = finally_exc_name,
    )));
    finally_body
}

pub(crate) fn prepare_except_body(
    handlers: &[ast::ExceptHandler],
    except_exc_name: &str,
) -> Vec<Box<Stmt>> {
    let except_body = handlers
        .first()
        .map(|handler| {
            let ast::ExceptHandler::ExceptHandler(handler) = handler;
            flatten_stmt_boxes(&handler.body.body)
        })
        .unwrap_or_else(|| vec![Box::new(py_stmt!("raise {exc:id}", exc = except_exc_name,))]);
    rewrite_exception_accesses_shared(except_body, except_exc_name)
}

pub(crate) struct LoweredLegacyTryRegions {
    pub body_label: String,
    pub except_label: String,
    pub body_region_range: std::ops::Range<usize>,
    pub except_region_range: std::ops::Range<usize>,
    pub finally_label: Option<String>,
    pub finally_region_labels: Vec<BlockPyLabel>,
    pub finally_fallthrough_label: Option<String>,
}

pub(crate) fn lower_legacy_try_regions<F>(
    blocks: &mut Vec<BlockPyBlock>,
    try_plan: &LegacyTryPlan,
    rest_entry: &str,
    finally_body: Option<Vec<Box<Stmt>>>,
    else_body: Vec<Box<Stmt>>,
    try_body: Vec<Box<Stmt>>,
    except_body: Vec<Box<Stmt>>,
    lower_region: &mut F,
) -> LoweredLegacyTryRegions
where
    F: FnMut(&[Box<Stmt>], String, &mut Vec<BlockPyBlock>) -> String,
{
    let (finally_label, finally_region_labels, finally_fallthrough_label) =
        if let Some(finally_body) = finally_body {
            let finally_region_start = blocks.len();
            let finally_label = lower_region(
                &finally_body,
                try_plan.finally_cont_label(rest_entry),
                blocks,
            );
            let finally_region_labels = collect_region_labels(&blocks[finally_region_start..]);
            if let (
                Some(finally_return_label),
                Some(finally_dispatch_label),
                Some(return_name),
                Some(reason_name),
            ) = (
                try_plan.finally_return_label.clone(),
                try_plan.finally_dispatch_label.clone(),
                try_plan.finally_return_value_name.as_ref(),
                try_plan.finally_reason_name.as_ref(),
            ) {
                emit_finally_return_dispatch_blocks(
                    blocks,
                    finally_return_label,
                    finally_dispatch_label,
                    return_name,
                    reason_name,
                    rest_entry.to_string(),
                );
            }
            (
                Some(finally_label),
                finally_region_labels,
                try_plan.finally_fallthrough_label(rest_entry),
            )
        } else {
            (None, Vec::new(), None)
        };

    let pass_target = try_plan.pass_target(finally_label.as_deref(), rest_entry);

    let body_region_start = blocks.len();
    emit_try_pass_block(
        blocks,
        try_plan.body_pass_label.clone(),
        try_plan.finally_reason_name.as_deref(),
        try_plan.finally_exc_name.as_deref(),
        None,
        pass_target.clone(),
    );
    let else_entry = lower_region(&else_body, try_plan.body_pass_label.clone(), blocks);
    let body_label = lower_region(&try_body, else_entry, blocks);
    let body_region_end = blocks.len();

    let except_region_start = blocks.len();
    emit_try_pass_block(
        blocks,
        try_plan.except_pass_label.clone(),
        try_plan.finally_reason_name.as_deref(),
        try_plan.finally_exc_name.as_deref(),
        Some(try_plan.except_exc_name.as_str()),
        pass_target,
    );
    let except_label = lower_region(&except_body, try_plan.except_pass_label.clone(), blocks);
    let except_region_end = blocks.len();

    LoweredLegacyTryRegions {
        body_label,
        except_label,
        body_region_range: body_region_start..body_region_end,
        except_region_range: except_region_start..except_region_end,
        finally_label,
        finally_region_labels,
        finally_fallthrough_label,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn finalize_legacy_try_regions(
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    linear: Vec<Stmt>,
    body_label: String,
    except_label: String,
    try_plan: LegacyTryPlan,
    body_region_range: std::ops::Range<usize>,
    except_region_range: std::ops::Range<usize>,
    finally_label: Option<String>,
    finally_region_labels: Vec<BlockPyLabel>,
    finally_fallthrough_label: Option<String>,
) -> String {
    if let (Some(reason_name), Some(return_name), Some(finally_target)) = (
        try_plan.finally_reason_name.as_ref(),
        try_plan.finally_return_value_name.as_ref(),
        finally_label.as_ref(),
    ) {
        let finally_exc_name = try_plan.finally_exc_name.as_deref();
        rewrite_region_returns_to_finally_blockpy_shared(
            &mut blocks[body_region_range.clone()],
            reason_name.as_str(),
            return_name.as_str(),
            finally_target.as_str(),
            finally_exc_name,
        );
        rewrite_region_returns_to_finally_blockpy_shared(
            &mut blocks[except_region_range.clone()],
            reason_name.as_str(),
            return_name.as_str(),
            finally_target.as_str(),
            finally_exc_name,
        );
    }

    emit_legacy_try_jump_entry(
        blocks,
        label,
        linear,
        body_label,
        except_label,
        try_plan.except_exc_name,
        collect_region_labels(&blocks[body_region_range]),
        collect_region_labels(&blocks[except_region_range]),
        finally_label,
        try_plan.finally_exc_name,
        finally_region_labels,
        finally_fallthrough_label,
    )
}

pub(crate) fn emit_try_pass_block(
    blocks: &mut Vec<BlockPyBlock>,
    pass_label: String,
    reason_name: Option<&str>,
    exc_name: Option<&str>,
    deleted_exc_name: Option<&str>,
    target_label: String,
) {
    let mut pass_stmts = Vec::new();
    if let Some(reason_name) = reason_name {
        pass_stmts.push(py_stmt!("{reason:id} = None", reason = reason_name,));
    }
    if let Some(exc_name) = exc_name {
        pass_stmts.push(py_stmt!("{exc:id} = None", exc = exc_name));
    }
    if let Some(deleted_exc_name) = deleted_exc_name {
        pass_stmts.push(py_stmt!("{exc:id} = __dp_DELETED", exc = deleted_exc_name,));
    }
    blocks.push(compat_block_from_blockpy(
        pass_label,
        pass_stmts,
        BlockPyStmt::Jump(BlockPyLabel::from(target_label)),
    ));
}

pub(crate) fn emit_finally_return_dispatch_blocks(
    blocks: &mut Vec<BlockPyBlock>,
    finally_return_label: String,
    finally_dispatch_label: String,
    return_name: &str,
    reason_name: &str,
    rest_entry: String,
) {
    blocks.push(compat_block_from_blockpy(
        finally_return_label.clone(),
        Vec::new(),
        BlockPyStmt::Return(Some(py_expr!("{name:id}", name = return_name).into())),
    ));
    blocks.push(compat_block_from_blockpy(
        finally_dispatch_label.clone(),
        Vec::new(),
        BlockPyStmt::If(BlockPyIf {
            test: py_expr!("__dp_eq({reason:id}, 'return')", reason = reason_name,).into(),
            body: vec![BlockPyBlock {
                label: BlockPyLabel::from(format!("{finally_dispatch_label}_true")),
                body: vec![BlockPyStmt::Jump(BlockPyLabel::from(finally_return_label))],
            }],
            orelse: vec![BlockPyBlock {
                label: BlockPyLabel::from(format!("{finally_dispatch_label}_false")),
                body: vec![BlockPyStmt::Jump(BlockPyLabel::from(rest_entry))],
            }],
        }),
    ));
}

pub(crate) fn collect_region_labels(blocks: &[BlockPyBlock]) -> Vec<BlockPyLabel> {
    blocks.iter().map(|block| block.label.clone()).collect()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_legacy_try_jump_entry(
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    linear: Vec<Stmt>,
    body_label: String,
    except_label: String,
    except_exc_name: String,
    body_region_labels: Vec<BlockPyLabel>,
    except_region_labels: Vec<BlockPyLabel>,
    finally_label: Option<String>,
    finally_exc_name: Option<String>,
    finally_region_labels: Vec<BlockPyLabel>,
    finally_fallthrough_label: Option<String>,
) -> String {
    blocks.push(compat_block_from_blockpy(
        label.clone(),
        linear,
        BlockPyStmt::LegacyTryJump(BlockPyLegacyTryJump {
            body_label: BlockPyLabel::from(body_label),
            except_label: BlockPyLabel::from(except_label),
            except_exc_name: Some(except_exc_name),
            body_region_labels,
            except_region_labels,
            finally_label: finally_label.map(BlockPyLabel::from),
            finally_exc_name,
            finally_region_labels,
            finally_fallthrough_label: finally_fallthrough_label.map(BlockPyLabel::from),
        }),
    ));
    label
}

fn block_references_label(block: &BlockPyBlock, label: &str) -> bool {
    fn stmt_references_label(stmt: &BlockPyStmt, label: &str) -> bool {
        match stmt {
            BlockPyStmt::Jump(target) => target.as_str() == label,
            BlockPyStmt::If(if_stmt) => {
                terminal_if_targets(if_stmt)
                    .map(|(then_label, else_label)| {
                        then_label.as_str() == label || else_label.as_str() == label
                    })
                    .unwrap_or(false)
                    || if_stmt
                        .body
                        .iter()
                        .chain(if_stmt.orelse.iter())
                        .any(|block| {
                            block
                                .body
                                .iter()
                                .any(|stmt| stmt_references_label(stmt, label))
                        })
            }
            BlockPyStmt::BranchTable(branch) => {
                branch.default_label.as_str() == label
                    || branch.targets.iter().any(|target| target.as_str() == label)
            }
            BlockPyStmt::Try(try_stmt) => try_stmt
                .body
                .iter()
                .chain(
                    try_stmt
                        .handlers
                        .iter()
                        .flat_map(|handler| handler.body.iter()),
                )
                .chain(try_stmt.orelse.iter())
                .chain(try_stmt.finalbody.iter())
                .any(|block| {
                    block
                        .body
                        .iter()
                        .any(|stmt| stmt_references_label(stmt, label))
                }),
            BlockPyStmt::LegacyTryJump(try_jump) => {
                try_jump.body_label.as_str() == label
                    || try_jump.except_label.as_str() == label
                    || try_jump
                        .finally_label
                        .as_ref()
                        .map(|target| target.as_str() == label)
                        .unwrap_or(false)
                    || try_jump
                        .finally_fallthrough_label
                        .as_ref()
                        .map(|target| target.as_str() == label)
                        .unwrap_or(false)
            }
            _ => false,
        }
    }

    block
        .body
        .iter()
        .any(|stmt| stmt_references_label(stmt, label))
}

fn terminal_if_targets(if_stmt: &BlockPyIf) -> Option<(&BlockPyLabel, &BlockPyLabel)> {
    let [BlockPyBlock {
        body: then_body, ..
    }] = if_stmt.body.as_slice()
    else {
        return None;
    };
    let [BlockPyStmt::Jump(then_label)] = then_body.as_slice() else {
        return None;
    };
    let [BlockPyBlock {
        body: else_body, ..
    }] = if_stmt.orelse.as_slice()
    else {
        return None;
    };
    let [BlockPyStmt::Jump(else_label)] = else_body.as_slice() else {
        return None;
    };
    Some((then_label, else_label))
}

fn relabel_generator_info(
    generator: &mut super::block_py::BlockPyGeneratorInfo,
    label_rename: &std::collections::HashMap<String, String>,
) {
    if let Some(dispatch_entry_label) = generator.dispatch_entry_label.as_mut() {
        if let Some(rewritten) = label_rename.get(dispatch_entry_label.as_str()) {
            *dispatch_entry_label = BlockPyLabel::from(rewritten.clone());
        }
    }
    for label in &mut generator.resume_order {
        if let Some(rewritten) = label_rename.get(label.as_str()) {
            *label = BlockPyLabel::from(rewritten.clone());
        }
    }
    for site in &mut generator.yield_sites {
        if let Some(rewritten) = label_rename.get(site.yield_label.as_str()) {
            site.yield_label = BlockPyLabel::from(rewritten.clone());
        }
        if let Some(rewritten) = label_rename.get(site.resume_label.as_str()) {
            site.resume_label = BlockPyLabel::from(rewritten.clone());
        }
    }
}

pub(crate) fn finalize_blockpy_function(
    mut function: BlockPyFunction,
    mut entry_label: String,
    end_label: String,
    label_prefix: &str,
    is_async_generator_runtime: bool,
    is_closure_backed_generator_runtime: bool,
    uncaught_exc_name: String,
) -> (BlockPyFunction, String) {
    let needs_end_block = entry_label == end_label
        || function
            .blocks
            .iter()
            .any(|block| block_references_label(block, end_label.as_str()));
    if needs_end_block {
        function.blocks.push(BlockPyBlock {
            label: BlockPyLabel::from(end_label),
            body: vec![BlockPyStmt::Return(None)],
        });
    }
    fold_jumps_to_trivial_none_return_blockpy(&mut function.blocks);
    fold_constant_brif_blockpy(&mut function.blocks);
    let prune_roots = function
        .generator
        .as_ref()
        .map(|info| {
            info.resume_order
                .iter()
                .map(|label| label.as_str().to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    prune_unreachable_blockpy_blocks(entry_label.as_str(), &prune_roots, &mut function.blocks);
    let (relabelled_entry_label, label_rename) =
        relabel_blockpy_blocks(label_prefix, entry_label.as_str(), &mut function.blocks);
    entry_label = relabelled_entry_label;
    if let Some(generator) = function.generator.as_mut() {
        relabel_generator_info(generator, &label_rename);
        *generator = synthesize_generator_dispatch_metadata(
            &mut function.blocks,
            &mut entry_label,
            label_prefix,
            is_async_generator_runtime,
            is_closure_backed_generator_runtime,
            uncaught_exc_name,
            &generator.resume_order,
            &generator.yield_sites,
        );
    }
    (function, entry_label)
}

#[derive(Clone)]
struct LoopContext {
    continue_label: BlockPyLabel,
    break_label: BlockPyLabel,
}

#[derive(Default)]
struct YieldLikeProbe {
    has_yield: bool,
    has_yield_from: bool,
}

impl Transformer for YieldLikeProbe {
    fn visit_stmt(&mut self, stmt: &mut Stmt) {
        if matches!(stmt, Stmt::FunctionDef(_) | Stmt::ClassDef(_)) {
            return;
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Yield(_) => self.has_yield = true,
            Expr::YieldFrom(_) => self.has_yield_from = true,
            _ => walk_expr(self, expr),
        }
    }
}

pub fn rewrite_ast_to_blockpy_module(
    module: &StmtBody,
    function_identity_by_node: &FunctionIdentityByNode,
) -> Result<BlockPyModule, String> {
    let mut module_out = BlockPyModule {
        prelude: Vec::new(),
        functions: Vec::new(),
        module_init: None,
    };
    let mut next_label_id = 0usize;

    for stmt in &module.body {
        match stmt.as_ref() {
            Stmt::FunctionDef(func) => {
                lower_function_recursive(
                    func,
                    function_identity_by_node,
                    &mut module_out.functions,
                    &mut module_out.module_init,
                )?;
            }
            stmt if is_ignorable_module_stmt(stmt) => {}
            other => {
                lower_stmt_into(other, &mut module_out.prelude, None, &mut next_label_id)?;
            }
        }
    }

    validate_final_blockpy_module(&module_out);

    Ok(module_out)
}

fn validate_final_blockpy_module(module: &BlockPyModule) {
    for stmt in &module.prelude {
        validate_no_live_yield_in_stmt(stmt);
    }
    for function in &module.functions {
        for block in &function.blocks {
            for stmt in &block.body {
                validate_no_live_yield_in_stmt(stmt);
            }
        }
    }
}

fn validate_no_live_yield_in_stmt(stmt: &BlockPyStmt) {
    match stmt {
        BlockPyStmt::Assign(assign) => validate_no_live_yield_in_expr(&assign.value),
        BlockPyStmt::Expr(expr) => validate_no_live_yield_in_expr(expr),
        BlockPyStmt::If(if_stmt) => {
            validate_no_live_yield_in_expr(&if_stmt.test);
            for block in &if_stmt.body {
                for stmt in &block.body {
                    validate_no_live_yield_in_stmt(stmt);
                }
            }
            for block in &if_stmt.orelse {
                for stmt in &block.body {
                    validate_no_live_yield_in_stmt(stmt);
                }
            }
        }
        BlockPyStmt::BranchTable(branch) => validate_no_live_yield_in_expr(&branch.index),
        BlockPyStmt::Return(value) => {
            if let Some(value) = value {
                validate_no_live_yield_in_expr(value);
            }
        }
        BlockPyStmt::Raise(raise_stmt) => {
            if let Some(exc) = &raise_stmt.exc {
                validate_no_live_yield_in_expr(exc);
            }
        }
        BlockPyStmt::Try(try_stmt) => {
            for block in &try_stmt.body {
                for stmt in &block.body {
                    validate_no_live_yield_in_stmt(stmt);
                }
            }
            for handler in &try_stmt.handlers {
                if let Some(type_) = &handler.type_ {
                    validate_no_live_yield_in_expr(type_);
                }
                for block in &handler.body {
                    for stmt in &block.body {
                        validate_no_live_yield_in_stmt(stmt);
                    }
                }
            }
            for block in &try_stmt.orelse {
                for stmt in &block.body {
                    validate_no_live_yield_in_stmt(stmt);
                }
            }
            for block in &try_stmt.finalbody {
                for stmt in &block.body {
                    validate_no_live_yield_in_stmt(stmt);
                }
            }
        }
        BlockPyStmt::Pass
        | BlockPyStmt::Delete(_)
        | BlockPyStmt::FunctionDef(_)
        | BlockPyStmt::Jump(_)
        | BlockPyStmt::LegacyTryJump(_) => {}
    }
}

fn validate_no_live_yield_in_expr(expr: &crate::basic_block::block_py::BlockPyExpr) {
    #[derive(Default)]
    struct YieldProbe {
        has_yield: bool,
    }

    impl Transformer for YieldProbe {
        fn visit_expr(&mut self, expr: &mut Expr) {
            match expr {
                Expr::Yield(_) | Expr::YieldFrom(_) => self.has_yield = true,
                _ => walk_expr(self, expr),
            }
        }
    }

    let mut probe = YieldProbe::default();
    let mut expr = expr.to_expr();
    probe.visit_expr(&mut expr);
    assert!(
        !probe.has_yield,
        "Yield/YieldFrom should be lowered before final BlockPy emission"
    );
}

fn lower_function_recursive(
    func: &ast::StmtFunctionDef,
    function_identity_by_node: &FunctionIdentityByNode,
    out: &mut Vec<BlockPyFunction>,
    module_init: &mut Option<String>,
) -> Result<(), String> {
    let Some((bind_name, _display_name, qualname, binding_target)) = function_identity_by_node
        .get(&func.node_index.load())
        .cloned()
    else {
        return Err(format!(
            "missing function identity for function {}\nstmt:\n{}",
            func.name.id,
            ruff_ast_to_string(&Stmt::FunctionDef(func.clone())).trim_end()
        ));
    };

    let lowered = lower_top_level_function(func, bind_name, qualname, binding_target)?;
    if lowered.bind_name == "_dp_module_init" {
        *module_init = Some(lowered.qualname.clone());
    }
    collect_nested_functions_from_blocks(
        &lowered.blocks,
        function_identity_by_node,
        out,
        module_init,
    )?;
    out.push(lowered);
    Ok(())
}

fn collect_nested_functions_from_blocks(
    blocks: &[BlockPyBlock],
    function_identity_by_node: &FunctionIdentityByNode,
    out: &mut Vec<BlockPyFunction>,
    module_init: &mut Option<String>,
) -> Result<(), String> {
    for block in blocks {
        collect_nested_functions_from_stmts(
            &block.body,
            function_identity_by_node,
            out,
            module_init,
        )?;
    }
    Ok(())
}

fn collect_nested_functions_from_stmts(
    stmts: &[BlockPyStmt],
    function_identity_by_node: &FunctionIdentityByNode,
    out: &mut Vec<BlockPyFunction>,
    module_init: &mut Option<String>,
) -> Result<(), String> {
    for stmt in stmts {
        match stmt {
            BlockPyStmt::FunctionDef(func) => {
                lower_function_recursive(func, function_identity_by_node, out, module_init)?;
            }
            BlockPyStmt::If(if_stmt) => {
                collect_nested_functions_from_blocks(
                    &if_stmt.body,
                    function_identity_by_node,
                    out,
                    module_init,
                )?;
                collect_nested_functions_from_blocks(
                    &if_stmt.orelse,
                    function_identity_by_node,
                    out,
                    module_init,
                )?;
            }
            BlockPyStmt::Try(try_stmt) => {
                collect_nested_functions_from_blocks(
                    &try_stmt.body,
                    function_identity_by_node,
                    out,
                    module_init,
                )?;
                for handler in &try_stmt.handlers {
                    collect_nested_functions_from_blocks(
                        &handler.body,
                        function_identity_by_node,
                        out,
                        module_init,
                    )?;
                }
                collect_nested_functions_from_blocks(
                    &try_stmt.orelse,
                    function_identity_by_node,
                    out,
                    module_init,
                )?;
                collect_nested_functions_from_blocks(
                    &try_stmt.finalbody,
                    function_identity_by_node,
                    out,
                    module_init,
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}

pub(crate) fn lower_stmts_to_blockpy_stmts(stmts: &[Stmt]) -> Result<Vec<BlockPyStmt>, String> {
    let mut out = Vec::new();
    let mut next_label_id = 0usize;
    for stmt in stmts {
        lower_stmt_into(stmt, &mut out, None, &mut next_label_id)?;
    }
    Ok(out)
}

fn generator_stmt_sequence_head(stmt: &Stmt) -> Option<(BlockPyStmt, bool)> {
    let generator_stmt = match lower_stmts_to_blockpy_stmts(std::slice::from_ref(stmt)) {
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
    let generator_stmt = generator_stmt
        .into_iter()
        .next()
        .expect("generator stmt conversion should yield one BlockPy stmt");
    let needs_rest_entry = blockpy_stmt_requires_generator_rest_entry(&generator_stmt);
    Some((generator_stmt, needs_rest_entry))
}

#[derive(Clone)]
pub(crate) struct GeneratorStmtSequencePlan {
    generator_stmt: BlockPyStmt,
    pub needs_rest_entry: bool,
}

pub(crate) fn plan_generator_stmt_in_sequence(stmt: &Stmt) -> Option<GeneratorStmtSequencePlan> {
    let (generator_stmt, needs_rest_entry) = generator_stmt_sequence_head(stmt)?;
    Some(GeneratorStmtSequencePlan {
        generator_stmt,
        needs_rest_entry,
    })
}

pub(crate) fn plan_stmt_sequence_head(stmt: &Stmt) -> StmtSequenceHeadPlan {
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

    match stmt {
        Stmt::Expr(_) | Stmt::Pass(_) | Stmt::Assign(_) => StmtSequenceHeadPlan::Linear(stmt.clone()),
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
    lower_non_bb_def: &mut FDef,
    rewrite_delete: &mut FDelete,
) -> StmtSequenceDriveResult
where
    FDef: FnMut(&ast::StmtFunctionDef) -> Vec<Stmt>,
    FDelete: FnMut(&ast::StmtDelete) -> Vec<Stmt>,
{
    let mut index = 0;
    while index < stmts.len() {
        match plan_stmt_sequence_head(stmts[index].as_ref()) {
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
            plan => return StmtSequenceDriveResult::Break { linear, index, plan },
        }
    }
    StmtSequenceDriveResult::Exhausted { linear }
}

fn compat_blockpy_raise_from_stmt(raise_stmt: ast::StmtRaise) -> BlockPyRaise {
    assert!(
        raise_stmt.cause.is_none(),
        "raise-from should be lowered before BlockPy construction"
    );
    BlockPyRaise {
        exc: raise_stmt.exc.map(|expr| (*expr).into()),
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
        StmtSequenceHeadPlan::With(with_stmt) => {
            let jump_label = if linear.is_empty() {
                None
            } else {
                Some(next_label())
            };
            Some(lower_with_stmt_sequence(
                with_stmt,
                remaining_stmts,
                cont_label,
                linear,
                blocks,
                jump_label,
                &mut |stmts, cont_label, blocks| lower_sequence(stmts, cont_label, None, blocks),
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

pub(crate) fn lower_try_stmt_sequence_head<F>(
    fn_name: &str,
    try_stmt: ast::StmtTry,
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    next_block_id: &Cell<usize>,
    lower_sequence: &mut F,
) -> String
where
    F: FnMut(&[Box<Stmt>], String, &mut Vec<BlockPyBlock>) -> String,
{
    let mut next_id = next_block_id.get();
    if try_stmt.is_star {
        let jump_label = (!linear.is_empty()).then(|| compat_next_label(fn_name, &mut next_id));
        next_block_id.set(next_id);
        return lower_star_try_stmt_sequence(
            try_stmt,
            remaining_stmts,
            cont_label,
            linear,
            blocks,
            jump_label,
            lower_sequence,
        );
    }

    let has_finally = !try_stmt.finalbody.body.is_empty();
    let needs_finally_return_flow = has_finally
        && (contains_return_stmt_in_body(&try_stmt.body.body)
            || contains_return_stmt_in_handlers(&try_stmt.handlers)
            || contains_return_stmt_in_body(&try_stmt.orelse.body));
    let try_plan = build_legacy_try_plan(
        fn_name,
        has_finally,
        needs_finally_return_flow,
        &mut next_id,
    );
    let label = compat_next_label(fn_name, &mut next_id);
    next_block_id.set(next_id);
    lower_legacy_try_stmt_sequence(
        try_stmt,
        remaining_stmts,
        cont_label,
        linear,
        blocks,
        label,
        try_plan,
        lower_sequence,
    )
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
    resume_order: &mut Vec<String>,
    yield_sites: &mut Vec<GeneratorYieldSite>,
    next_block_id: &mut usize,
    fn_name: &str,
    cell_slots: Option<&std::collections::HashSet<String>>,
) -> Option<String> {
    lower_generator_blockpy_stmt_in_sequence(
        &plan.generator_stmt,
        linear,
        rest_entry,
        blocks,
        closure_state,
        resume_order,
        yield_sites,
        next_block_id,
        fn_name,
        cell_slots,
    )
}

pub(crate) fn lower_generator_stmt_sequence_head<F>(
    fn_name: &str,
    plan: GeneratorStmtSequencePlan,
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    mut state: GeneratorStmtSequenceLoweringState,
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
        &mut state.resume_order,
        &mut state.yield_sites,
        &mut state.next_block_id,
        fn_name,
        cell_slots,
    );
    (label, state)
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
        BlockPyStmt::Jump(BlockPyLabel::from(expanded_entry)),
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

fn lower_top_level_function(
    func: &ast::StmtFunctionDef,
    bind_name: String,
    qualname: String,
    binding_target: BindingTarget,
) -> Result<BlockPyFunction, String> {
    let mut next_label_id = 0usize;
    let kind = function_kind_from_def(func);
    let mut runtime_body = func.body.clone();
    let coroutine_via_generator = matches!(kind, BlockPyFunctionKind::Coroutine);
    if func.is_async {
        lower_coroutine_awaits_to_yield_from(&mut runtime_body.body);
        if coroutine_via_generator && !body_has_yieldlike(&runtime_body) {
            runtime_body
                .body
                .insert(0, coroutine_generator_marker_stmt());
        }
    }
    let mut blocks = lower_body_to_blocks_with_entry(
        &runtime_body,
        BlockPyLabel::from("start"),
        None,
        &mut next_label_id,
    )?;
    let has_generator_runtime = matches!(
        kind,
        BlockPyFunctionKind::Generator | BlockPyFunctionKind::AsyncGenerator
    ) || coroutine_via_generator;
    let mut generator = None;
    if has_generator_runtime {
        let is_generated_genexpr = func.name.id.as_str().contains("_dp_genexpr_");
        let is_generated_comprehension_helper = is_generated_genexpr
            || func.name.id.as_str().contains("_dp_listcomp_")
            || func.name.id.as_str().contains("_dp_setcomp_")
            || func.name.id.as_str().contains("_dp_dictcomp_");
        let closure_state = !(is_generated_comprehension_helper && func.is_async);
        let BlockPyGeneratorLoweringResult {
            blocks: lowered_blocks,
            info,
        } = lower_generator_blockpy_blocks(
            func.name.id.as_str(),
            blocks,
            closure_state,
            matches!(kind, BlockPyFunctionKind::AsyncGenerator),
            &mut next_label_id,
        );
        blocks = lowered_blocks;
        generator = info;
    }
    Ok(build_blockpy_function(
        bind_name,
        qualname,
        binding_target,
        kind,
        (*func.parameters).clone(),
        blocks,
        generator,
    ))
}

fn body_has_yieldlike(body: &StmtBody) -> bool {
    let mut probe = YieldLikeProbe::default();
    for stmt in &body.body {
        let mut stmt = stmt.as_ref().clone();
        probe.visit_stmt(&mut stmt);
        if probe.has_yield || probe.has_yield_from {
            return true;
        }
    }
    false
}

fn lower_nested_body_to_blocks(
    body: &StmtBody,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
    label_prefix: &str,
) -> Result<Vec<BlockPyBlock>, String> {
    lower_body_to_blocks_with_entry(
        body,
        fresh_blockpy_label(label_prefix, next_label_id),
        loop_ctx,
        next_label_id,
    )
}

fn lower_stmt_into(
    stmt: &Stmt,
    out: &mut Vec<BlockPyStmt>,
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
        Stmt::Pass(_) => out.push(BlockPyStmt::Pass),
        Stmt::Expr(expr_stmt) => out.push(BlockPyStmt::Expr((*expr_stmt.value).clone().into())),
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
            out.push(BlockPyStmt::Assign(BlockPyAssign {
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
            out.push(BlockPyStmt::Delete(BlockPyDelete { target }));
        }
        Stmt::FunctionDef(func) => {
            out.push(BlockPyStmt::FunctionDef(func.clone()));
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
            let body =
                lower_nested_body_to_blocks(&if_stmt.body, loop_ctx, next_label_id, "if_body")?;
            let orelse =
                lower_orelse_to_blocks(&if_stmt.elif_else_clauses, stmt, loop_ctx, next_label_id)?;
            out.push(BlockPyStmt::If(BlockPyIf {
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
                out.push(BlockPyStmt::Jump(loop_ctx.break_label.clone()));
            } else {
                panic!("Break should be lowered before Ruff AST -> BlockPy conversion");
            }
        }
        Stmt::Continue(_) => {
            if let Some(loop_ctx) = loop_ctx {
                out.push(BlockPyStmt::Jump(loop_ctx.continue_label.clone()));
            } else {
                panic!("Continue should be lowered before Ruff AST -> BlockPy conversion");
            }
        }
        Stmt::Return(return_stmt) => {
            out.push(BlockPyStmt::Return(
                return_stmt.value.as_ref().map(|v| (**v).clone().into()),
            ));
        }
        Stmt::Raise(raise_stmt) => {
            if raise_stmt.cause.is_some() {
                panic!("raise-from should be lowered before Ruff AST -> BlockPy conversion");
            }
            out.push(BlockPyStmt::Raise(BlockPyRaise {
                exc: raise_stmt.exc.as_ref().map(|exc| (**exc).clone().into()),
            }));
        }
        Stmt::Try(try_stmt) => {
            let handler_kind = if try_stmt.is_star {
                BlockPyExceptHandlerKind::ExceptStar
            } else {
                BlockPyExceptHandlerKind::Except
            };
            let handlers = try_stmt
                .handlers
                .iter()
                .enumerate()
                .map(|(index, handler)| {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    Ok(BlockPyExceptHandler {
                        kind: handler_kind,
                        type_: handler.type_.as_ref().map(|expr| (**expr).clone().into()),
                        name: handler.name.as_ref().map(|name| name.id.to_string()),
                        body: lower_nested_body_to_blocks(
                            &handler.body,
                            loop_ctx,
                            next_label_id,
                            &format!("except_{index}"),
                        )?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            out.push(BlockPyStmt::Try(BlockPyTry {
                body: lower_nested_body_to_blocks(
                    &try_stmt.body,
                    loop_ctx,
                    next_label_id,
                    "try_body",
                )?,
                handlers,
                orelse: lower_nested_body_to_blocks(
                    &try_stmt.orelse,
                    loop_ctx,
                    next_label_id,
                    "try_else",
                )?,
                finalbody: lower_nested_body_to_blocks(
                    &try_stmt.finalbody,
                    loop_ctx,
                    next_label_id,
                    "try_finally",
                )?,
            }));
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
    with_stmt: ast::StmtWith,
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
    let lowered_with = desugar_with_stmt_for_bb(with_stmt);
    lower_expanded_stmt_sequence(
        lowered_with,
        remaining_stmts,
        cont_label,
        linear,
        blocks,
        jump_label,
        lower_sequence,
    )
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

pub(crate) fn lower_legacy_try_stmt_sequence<F>(
    try_stmt: ast::StmtTry,
    remaining_stmts: &[Box<Stmt>],
    cont_label: String,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    label: String,
    try_plan: LegacyTryPlan,
    lower_sequence: &mut F,
) -> String
where
    F: FnMut(&[Box<Stmt>], String, &mut Vec<BlockPyBlock>) -> String,
{
    let rest_entry = lower_sequence(remaining_stmts, cont_label.clone(), blocks);

    let else_body = flatten_stmt_boxes(&try_stmt.orelse.body);
    let try_body = flatten_stmt_boxes(&try_stmt.body.body);
    let except_body = prepare_except_body(&try_stmt.handlers, try_plan.except_exc_name.as_str());
    let finally_body = if !try_stmt.finalbody.body.is_empty() {
        let finally_exc_candidate = try_plan
            .finally_exc_name
            .as_ref()
            .expect("try/finally planning should allocate exception temp");
        Some(prepare_finally_body(
            &try_stmt.finalbody,
            finally_exc_candidate.as_str(),
        ))
    } else {
        None
    };

    let lowered_try = lower_legacy_try_regions(
        blocks,
        &try_plan,
        rest_entry.as_str(),
        finally_body,
        else_body,
        try_body,
        except_body,
        lower_sequence,
    );

    finalize_legacy_try_regions(
        blocks,
        label,
        linear,
        lowered_try.body_label,
        lowered_try.except_label,
        try_plan,
        lowered_try.body_region_range,
        lowered_try.except_region_range,
        lowered_try.finally_label,
        lowered_try.finally_region_labels,
        lowered_try.finally_fallthrough_label,
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

fn lower_generated_stmts_into_blockpy(
    stmts: Vec<Stmt>,
    out: &mut Vec<BlockPyStmt>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<(), String> {
    for stmt in lower_coroutine_awaits_in_stmts(stmts) {
        lower_stmt_into(&stmt, out, loop_ctx, next_label_id)?;
    }
    Ok(())
}

fn assignment_target_body(
    target: Expr,
    rhs: Expr,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<Vec<BlockPyStmt>, String> {
    let mut stmts = Vec::new();
    let mut next_temp = |prefix: &str| fresh_name(prefix);
    rewrite_assignment_target(target, rhs, &mut stmts, &mut next_temp);
    let mut out = Vec::new();
    lower_generated_stmts_into_blockpy(stmts, &mut out, loop_ctx, next_label_id)?;
    Ok(out)
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

pub(crate) fn desugar_with_stmt_for_bb(with_stmt: ast::StmtWith) -> Stmt {
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
        let suppress_name = fresh_name("with_suppress");
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
    {suppress_name:id} = await __dp_asynccontextmanager_aexit({exit_name:id}, __dp_exc_info())
    if not {suppress_name:id}:
        raise
finally:
    if {ok_name:id}:
        await __dp_asynccontextmanager_aexit({exit_name:id}, None)
    {exit_name:id} = None
    {ctx_cleanup:stmt}
"#,
                ctx_placeholder_stmt = ctx_placeholder_stmt,
                ctx_expr = ctx_expr.clone(),
                enter_stmt = enter_stmt,
                body = lowered_body,
                exit_name = exit_name.as_str(),
                ok_name = ok_name.as_str(),
                suppress_name = suppress_name.as_str(),
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

fn lower_with_into(
    with_stmt: ast::StmtWith,
    out: &mut Vec<BlockPyStmt>,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<(), String> {
    let lowered_body = desugar_with_stmt_for_bb(with_stmt);
    lower_stmt_into(&lowered_body, out, loop_ctx, next_label_id)
}

fn lower_body_to_blocks_with_entry(
    body: &StmtBody,
    entry_label: BlockPyLabel,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<Vec<BlockPyBlock>, String> {
    let mut blocks = Vec::new();
    let mut current_label = entry_label;
    let mut current_body = Vec::new();

    for stmt in &body.body {
        match stmt.as_ref() {
            Stmt::While(while_stmt) => {
                blocks.push(BlockPyBlock {
                    label: current_label.clone(),
                    body: current_body,
                });

                let test_label = fresh_blockpy_label("while_test", next_label_id);
                let body_label = fresh_blockpy_label("while_body", next_label_id);
                let after_label = fresh_blockpy_label("while_after", next_label_id);
                let else_label = if while_stmt.orelse.body.is_empty() {
                    None
                } else {
                    Some(fresh_blockpy_label("while_else", next_label_id))
                };

                blocks.push(BlockPyBlock {
                    label: test_label.clone(),
                    body: vec![BlockPyStmt::If(BlockPyIf {
                        test: (*while_stmt.test).clone().into(),
                        body: vec![BlockPyBlock {
                            label: fresh_blockpy_label("while_if_true", next_label_id),
                            body: vec![BlockPyStmt::Jump(body_label.clone())],
                        }],
                        orelse: vec![BlockPyBlock {
                            label: fresh_blockpy_label("while_if_false", next_label_id),
                            body: vec![BlockPyStmt::Jump(
                                else_label.clone().unwrap_or_else(|| after_label.clone()),
                            )],
                        }],
                    })],
                });

                let inner_loop_ctx = LoopContext {
                    continue_label: test_label.clone(),
                    break_label: after_label.clone(),
                };
                let mut loop_body = lower_body_to_blocks_with_entry(
                    &while_stmt.body,
                    body_label.clone(),
                    Some(&inner_loop_ctx),
                    next_label_id,
                )?;
                ensure_terminal_jump(&mut loop_body, test_label.clone());
                blocks.extend(loop_body);

                if let Some(else_label) = else_label {
                    blocks.extend(lower_body_to_blocks_with_entry(
                        &while_stmt.orelse,
                        else_label,
                        loop_ctx,
                        next_label_id,
                    )?);
                }

                current_label = after_label;
                current_body = Vec::new();
            }
            Stmt::For(for_stmt) => {
                blocks.push(BlockPyBlock {
                    label: current_label.clone(),
                    body: current_body,
                });

                let setup_label = fresh_blockpy_label("for_setup", next_label_id);
                let fetch_label = fresh_blockpy_label("for_fetch", next_label_id);
                let body_label = fresh_blockpy_label("for_body", next_label_id);
                let after_label = fresh_blockpy_label("for_after", next_label_id);
                let else_label = if for_stmt.orelse.body.is_empty() {
                    None
                } else {
                    Some(fresh_blockpy_label("for_else", next_label_id))
                };
                let iter_name = fresh_name("iter");
                let target_tmp = fresh_name("tmp");

                let iter_expr = if for_stmt.is_async {
                    py_expr!("__dp_aiter({iter:expr})", iter = *for_stmt.iter.clone())
                } else {
                    py_expr!("__dp_iter({iter:expr})", iter = *for_stmt.iter.clone())
                };
                blocks.push(BlockPyBlock {
                    label: setup_label.clone(),
                    body: vec![BlockPyStmt::Assign(BlockPyAssign {
                        target: py_expr!("{name:id}", name = iter_name.as_str())
                            .as_name_expr()
                            .expect("fresh iter temp should be a name")
                            .clone(),
                        value: iter_expr.into(),
                    })],
                });

                let fetch_value = if for_stmt.is_async {
                    py_expr!(
                        "await __dp_anext_or_sentinel({iter:expr})",
                        iter = py_expr!("{name:id}", name = iter_name.as_str())
                    )
                } else {
                    py_expr!(
                        "__dp_next_or_sentinel({iter:expr})",
                        iter = py_expr!("{name:id}", name = iter_name.as_str())
                    )
                };

                let mut false_body = assignment_target_body(
                    *for_stmt.target.clone(),
                    py_expr!("{tmp:id}", tmp = target_tmp.as_str()),
                    None,
                    next_label_id,
                )?;
                false_body.push(BlockPyStmt::Assign(BlockPyAssign {
                    target: py_expr!("{tmp:id}", tmp = target_tmp.as_str())
                        .as_name_expr()
                        .expect("fresh target temp should be a name")
                        .clone(),
                    value: py_expr!("None").into(),
                }));
                false_body.push(BlockPyStmt::Jump(body_label.clone()));

                blocks.push(BlockPyBlock {
                    label: fetch_label.clone(),
                    body: vec![
                        BlockPyStmt::Assign(BlockPyAssign {
                            target: py_expr!("{tmp:id}", tmp = target_tmp.as_str())
                                .as_name_expr()
                                .expect("fresh target temp should be a name")
                                .clone(),
                            value: fetch_value.into(),
                        }),
                        BlockPyStmt::If(BlockPyIf {
                            test: py_expr!(
                                "__dp_is_({value:expr}, __dp__.ITER_COMPLETE)",
                                value = py_expr!("{tmp:id}", tmp = target_tmp.as_str())
                            )
                            .into(),
                            body: vec![BlockPyBlock {
                                label: fresh_blockpy_label("for_done", next_label_id),
                                body: vec![BlockPyStmt::Jump(
                                    else_label.clone().unwrap_or_else(|| after_label.clone()),
                                )],
                            }],
                            orelse: vec![BlockPyBlock {
                                label: fresh_blockpy_label("for_next", next_label_id),
                                body: false_body,
                            }],
                        }),
                    ],
                });

                let inner_loop_ctx = LoopContext {
                    continue_label: fetch_label.clone(),
                    break_label: after_label.clone(),
                };
                let mut loop_body = lower_body_to_blocks_with_entry(
                    &for_stmt.body,
                    body_label.clone(),
                    Some(&inner_loop_ctx),
                    next_label_id,
                )?;
                ensure_terminal_jump(&mut loop_body, fetch_label.clone());
                blocks.extend(loop_body);

                if let Some(else_label) = else_label {
                    blocks.extend(lower_body_to_blocks_with_entry(
                        &for_stmt.orelse,
                        else_label,
                        loop_ctx,
                        next_label_id,
                    )?);
                }

                current_label = after_label;
                current_body = Vec::new();
            }
            _ => lower_stmt_into(stmt.as_ref(), &mut current_body, loop_ctx, next_label_id)?,
        }
    }

    blocks.push(BlockPyBlock {
        label: current_label,
        body: current_body,
    });
    Ok(blocks)
}

fn fresh_blockpy_label(prefix: &str, next_label_id: &mut usize) -> BlockPyLabel {
    let label = BlockPyLabel::from(format!("{prefix}_{next_label_id}"));
    *next_label_id += 1;
    label
}

fn ensure_terminal_jump(blocks: &mut [BlockPyBlock], target: BlockPyLabel) {
    let Some(last) = blocks.last_mut() else {
        return;
    };
    if !last.body.last().is_some_and(is_terminal_stmt) {
        last.body.push(BlockPyStmt::Jump(target));
    }
}

fn is_terminal_stmt(stmt: &BlockPyStmt) -> bool {
    matches!(
        stmt,
        BlockPyStmt::Jump(_)
            | BlockPyStmt::If(_)
            | BlockPyStmt::BranchTable(_)
            | BlockPyStmt::Raise(_)
            | BlockPyStmt::LegacyTryJump(_)
            | BlockPyStmt::Return(_)
    )
}

fn lower_orelse_to_blocks(
    clauses: &[ast::ElifElseClause],
    stmt: &Stmt,
    loop_ctx: Option<&LoopContext>,
    next_label_id: &mut usize,
) -> Result<Vec<BlockPyBlock>, String> {
    match clauses {
        [] => Ok(Vec::new()),
        [clause] if clause.test.is_none() => {
            lower_nested_body_to_blocks(&clause.body, loop_ctx, next_label_id, "if_else")
        }
        _ => Err(format!(
            "`elif` chain reached Ruff AST -> BlockPy conversion\nstmt:\n{}",
            ruff_ast_to_string(stmt).trim_end()
        )),
    }
}

fn is_ignorable_module_stmt(stmt: &Stmt) -> bool {
    matches!(
        stmt,
        Stmt::Expr(ast::StmtExpr { value, .. }) if matches!(value.as_ref(), Expr::StringLiteral(_))
    ) || matches!(
        stmt,
        Stmt::ImportFrom(ast::StmtImportFrom { module, .. })
            if module.as_ref().map(|name| name.id.as_str()) == Some("__future__")
    ) || matches!(stmt, Stmt::Pass(_))
}

fn function_kind_from_def(func: &ast::StmtFunctionDef) -> BlockPyFunctionKind {
    let mut probe = YieldLikeProbe::default();
    for stmt in &func.body.body {
        let mut stmt = stmt.as_ref().clone();
        probe.visit_stmt(&mut stmt);
        if probe.has_yield || probe.has_yield_from {
            break;
        }
    }
    match (func.is_async, probe.has_yield || probe.has_yield_from) {
        (true, true) => BlockPyFunctionKind::AsyncGenerator,
        (true, false) => BlockPyFunctionKind::Coroutine,
        (false, true) => BlockPyFunctionKind::Generator,
        (false, false) => BlockPyFunctionKind::Function,
    }
}

fn stmt_kind_name(stmt: &Stmt) -> &'static str {
    match stmt {
        Stmt::FunctionDef(_) => "FunctionDef",
        Stmt::ClassDef(_) => "ClassDef",
        Stmt::Return(_) => "Return",
        Stmt::Delete(_) => "Delete",
        Stmt::TypeAlias(_) => "TypeAlias",
        Stmt::Assign(_) => "Assign",
        Stmt::AugAssign(_) => "AugAssign",
        Stmt::AnnAssign(_) => "AnnAssign",
        Stmt::For(_) => "For",
        Stmt::While(_) => "While",
        Stmt::If(_) => "If",
        Stmt::With(_) => "With",
        Stmt::Match(_) => "Match",
        Stmt::Raise(_) => "Raise",
        Stmt::Try(_) => "Try",
        Stmt::Assert(_) => "Assert",
        Stmt::Import(_) => "Import",
        Stmt::ImportFrom(_) => "ImportFrom",
        Stmt::Global(_) => "Global",
        Stmt::Nonlocal(_) => "Nonlocal",
        Stmt::Expr(_) => "Expr",
        Stmt::Pass(_) => "Pass",
        Stmt::Break(_) => "Break",
        Stmt::Continue(_) => "Continue",
        Stmt::IpyEscapeCommand(_) => "IpyEscapeCommand",
        Stmt::BodyStmt(_) => "BodyStmt",
    }
}

fn assign_delete_error(message: &str, stmt: &Stmt) -> String {
    format!("{message}\nstmt:\n{}", ruff_ast_to_string(stmt).trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basic_block::bb_ir::BindingTarget;

    fn function_identity(stmt: &ast::StmtFunctionDef) -> FunctionIdentityByNode {
        FunctionIdentityByNode::from([(
            stmt.node_index.load(),
            (
                stmt.name.id.to_string(),
                stmt.name.id.to_string(),
                stmt.name.id.to_string(),
                BindingTarget::Local,
            ),
        )])
    }

    #[test]
    fn lowers_post_simplification_control_flow() {
        let module = ruff_python_parser::parse_module(
            r#"
def f(x, ys):
    while x:
        for y in ys:
            if y:
                break
            continue
    try:
        return x
    except ValueError as err:
        return err
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let blockpy = rewrite_ast_to_blockpy_module(&module, &function_identity(&func)).unwrap();
        assert_eq!(blockpy.functions.len(), 1);
        let blocks = &blockpy.functions[0].blocks;
        assert!(blocks
            .iter()
            .any(|block| matches!(block.body.first(), Some(BlockPyStmt::If(_)))));
        assert!(blocks.iter().any(|block| {
            matches!(
                block.body.first(),
                Some(BlockPyStmt::Assign(_)) if block.label.as_str().starts_with("for_setup_")
            )
        }));
        assert!(matches!(
            blocks.last().unwrap().body.first(),
            Some(BlockPyStmt::Try(_))
        ));
    }

    #[test]
    fn lowers_async_for_structurally() {
        let module = ruff_python_parser::parse_module(
            r#"
async def f(xs):
    async for x in xs:
        body(x)
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let blockpy = rewrite_ast_to_blockpy_module(&module, &function_identity(&func)).unwrap();
        let blocks = &blockpy.functions[0].blocks;
        assert!(blocks.iter().any(|block| {
            matches!(
                block.body.first(),
                Some(BlockPyStmt::Assign(_)) if block.label.as_str().starts_with("for_setup_")
            )
        }));
        assert!(blocks
            .iter()
            .any(|block| block.label.as_str().starts_with("for_fetch_")));
    }

    #[test]
    fn lowers_generator_yield_to_explicit_blockpy_dispatch() {
        let module = ruff_python_parser::parse_module(
            r#"
def gen(n):
    yield n
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let blockpy = rewrite_ast_to_blockpy_module(&module, &function_identity(&func)).unwrap();
        let rendered = crate::basic_block::blockpy_module_to_string(&blockpy);
        assert!(rendered.contains("branch_table"));
        assert!(!rendered.contains("yield n"));
    }

    #[test]
    fn shared_generator_stmt_wrapper_lowers_yield_stmt() {
        let module = ruff_python_parser::parse_module(
            r#"
def gen(n):
    yield n
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = func
            .body
            .body
            .first()
            .expect("expected generator body stmt")
            .as_ref()
            .clone();

        let mut blocks = Vec::new();
        let mut resume_order = Vec::new();
        let mut yield_sites = Vec::new();
        let mut next_block_id = 0usize;
        let plan =
            plan_generator_stmt_in_sequence(&stmt).expect("expected generator stmt sequence plan");
        assert!(plan.needs_rest_entry);
        let label = lower_generator_stmt_sequence_plan(
            &plan,
            Vec::new(),
            Some("cont".to_string()),
            &mut blocks,
            false,
            &mut resume_order,
            &mut yield_sites,
            &mut next_block_id,
            "gen",
            None,
        );

        assert!(label.is_some());
        assert!(!blocks.is_empty());
        assert_eq!(yield_sites.len(), 1);
    }

    #[test]
    fn generator_sequence_head_marks_yield_stmt_as_needing_rest() {
        let module = ruff_python_parser::parse_module(
            r#"
def gen(n):
    yield n
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = func
            .body
            .body
            .first()
            .expect("expected generator body stmt")
            .as_ref()
            .clone();

        let needs_rest_entry = plan_generator_stmt_in_sequence(&stmt)
            .expect("expected generator sequence head")
            .needs_rest_entry;

        assert!(needs_rest_entry);
    }

    #[test]
    fn generator_sequence_head_marks_return_yield_as_not_needing_rest() {
        let module = ruff_python_parser::parse_module(
            r#"
def gen(n):
    return (yield n)
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = func
            .body
            .body
            .first()
            .expect("expected generator body stmt")
            .as_ref()
            .clone();

        let needs_rest_entry = plan_generator_stmt_in_sequence(&stmt)
            .expect("expected generator sequence head")
            .needs_rest_entry;

        assert!(!needs_rest_entry);
    }

    #[test]
    fn stmt_sequence_head_plan_marks_yield_expr_as_generator() {
        let module = ruff_python_parser::parse_module(
            r#"
def gen():
    yield x
"#,
        )
            .unwrap()
            .into_syntax()
            .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = func.body.body[0].as_ref();

        assert!(matches!(
            plan_stmt_sequence_head(stmt),
            StmtSequenceHeadPlan::Generator {
                sync_target_cells: false,
                ..
            }
        ));
    }

    #[test]
    fn stmt_sequence_head_plan_marks_assign_yield_as_generator_with_cell_sync() {
        let module = ruff_python_parser::parse_module(
            r#"
def gen():
    x = (yield y)
"#,
        )
            .unwrap()
            .into_syntax()
            .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = func.body.body[0].as_ref();

        assert!(matches!(
            plan_stmt_sequence_head(stmt),
            StmtSequenceHeadPlan::Generator {
                sync_target_cells: true,
                ..
            }
        ));
    }

    #[test]
    fn stmt_sequence_head_plan_marks_return_without_yield_as_generator_head() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    return x
"#,
        )
            .unwrap()
            .into_syntax()
            .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = func.body.body[0].as_ref();

        assert!(matches!(
            plan_stmt_sequence_head(stmt),
            StmtSequenceHeadPlan::Generator {
                sync_target_cells: false,
                ..
            }
        ));
    }

    #[test]
    fn prepare_generator_stmt_plan_from_stmt_only_lowers_rest_when_needed() {
        let module = ruff_python_parser::parse_module(
            r#"
def gen(n):
    yield n
    after()
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = func.body.body[0].as_ref();
        let remaining = vec![func.body.body[1].clone()];

        let mut blocks = Vec::new();
        let mut rest_calls = Vec::new();

        let generator_plan =
            plan_generator_stmt_in_sequence(stmt).expect("expected generator stmt sequence plan");
        let (label, state) = lower_generator_stmt_sequence_head(
            "gen",
            generator_plan,
            &remaining,
            "cont".to_string(),
            Vec::new(),
            &mut blocks,
            GeneratorStmtSequenceLoweringState {
                closure_state: false,
                resume_order: Vec::new(),
                yield_sites: Vec::new(),
                next_block_id: 0,
            },
            None,
            &mut |stmts, cont_label, _blocks, state| {
                rest_calls.push((stmts.len(), cont_label.clone()));
                ("rest_entry".to_string(), state)
            },
        );

        assert_eq!(rest_calls, vec![(1, "cont".to_string())]);
        assert!(label.is_some());
        assert_eq!(state.yield_sites.len(), 1);
    }

    #[test]
    fn prepare_generator_stmt_plan_from_stmt_skips_rest_when_not_needed() {
        let module = ruff_python_parser::parse_module(
            r#"
def gen(n):
    return (yield n)
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = func.body.body[0].as_ref();

        let mut blocks = Vec::new();
        let mut rest_called = false;

        let generator_plan =
            plan_generator_stmt_in_sequence(stmt).expect("expected generator stmt sequence plan");
        let (label, state) = lower_generator_stmt_sequence_head(
            "gen",
            generator_plan,
            &[],
            "cont".to_string(),
            Vec::new(),
            &mut blocks,
            GeneratorStmtSequenceLoweringState {
                closure_state: false,
                resume_order: Vec::new(),
                yield_sites: Vec::new(),
                next_block_id: 0,
            },
            None,
            &mut |_stmts, _cont_label, _blocks, state| {
                rest_called = true;
                ("rest_entry".to_string(), state)
            },
        );

        assert!(!rest_called);
        assert!(label.is_some());
        assert_eq!(state.yield_sites.len(), 1);
    }

    #[test]
    fn lower_for_loop_continue_entry_with_state_returns_loop_check_for_sync_for() {
        let mut blocks = Vec::new();
        let (entry, state) = lower_for_loop_continue_entry_with_state(
            &mut blocks,
            "demo",
            "_dp_iter_0",
            "_dp_tmp_0",
            "_dp_bb_demo_0".to_string(),
            false,
            GeneratorStmtSequenceLoweringState {
                closure_state: false,
                resume_order: Vec::new(),
                yield_sites: Vec::new(),
                next_block_id: 0,
            },
        );

        assert_eq!(entry, "_dp_bb_demo_0");
        assert!(blocks.is_empty());
        assert!(state.resume_order.is_empty());
        assert!(state.yield_sites.is_empty());
        assert_eq!(state.next_block_id, 0);
    }

    #[test]
    fn lower_for_loop_continue_entry_with_state_updates_async_generator_state() {
        let mut blocks = Vec::new();
        let (entry, state) = lower_for_loop_continue_entry_with_state(
            &mut blocks,
            "demo",
            "_dp_iter_0",
            "_dp_tmp_0",
            "_dp_bb_demo_0".to_string(),
            true,
            GeneratorStmtSequenceLoweringState {
                closure_state: false,
                resume_order: Vec::new(),
                yield_sites: Vec::new(),
                next_block_id: 0,
            },
        );

        assert!(!blocks.is_empty());
        assert_ne!(entry, "_dp_bb_demo_0");
        assert!(!state.resume_order.is_empty());
        assert_eq!(state.yield_sites.len(), 1);
        assert!(state.next_block_id > 0);
    }

    #[test]
    fn lower_for_stmt_sequence_emits_loop_scaffolding() {
        let module = ruff_python_parser::parse_module(
            r#"
def f(xs):
    for x in xs:
        body(x)
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let ast::Stmt::For(for_stmt) = func.body.body[0].as_ref() else {
            panic!("expected for stmt");
        };

        let mut blocks = Vec::new();
        let entry = lower_for_stmt_sequence(
            for_stmt.clone(),
            &[],
            "cont".to_string(),
            Vec::new(),
            &mut blocks,
            "_dp_iter_0",
            "_dp_tmp_0",
            "_dp_bb_demo_0".to_string(),
            "_dp_bb_demo_0".to_string(),
            "_dp_bb_demo_1".to_string(),
            "_dp_bb_demo_2".to_string(),
            vec![py_stmt!("x = _dp_tmp_0"), py_stmt!("_dp_tmp_0 = None")],
            &mut |_stmts, cont_label, _break_label, _blocks| cont_label,
        );

        assert_eq!(entry, "_dp_bb_demo_2");
        assert!(blocks
            .iter()
            .any(|block| block.label.as_str() == "_dp_bb_demo_1"));
        assert!(blocks
            .iter()
            .any(|block| block.label.as_str() == "_dp_bb_demo_2"));
    }

    #[test]
    fn builds_closure_backed_generator_factory_block() {
        let layout = crate::basic_block::bb_ir::BbGeneratorClosureLayout {
            inherited_captures: vec![crate::basic_block::bb_ir::BbGeneratorClosureSlot {
                logical_name: "captured".to_string(),
                storage_name: "_dp_cell_captured".to_string(),
                init: crate::basic_block::bb_ir::BbGeneratorClosureInit::InheritedCapture,
            }],
            lifted_locals: vec![crate::basic_block::bb_ir::BbGeneratorClosureSlot {
                logical_name: "x".to_string(),
                storage_name: "_dp_cell_x".to_string(),
                init: crate::basic_block::bb_ir::BbGeneratorClosureInit::Parameter,
            }],
            runtime_cells: vec![crate::basic_block::bb_ir::BbGeneratorClosureSlot {
                logical_name: "_dp_pc".to_string(),
                storage_name: "_dp_cell__dp_pc".to_string(),
                init: crate::basic_block::bb_ir::BbGeneratorClosureInit::RuntimePcZero,
            }],
        };

        let block = build_closure_backed_generator_factory_block(
            "_dp_bb_demo_factory",
            "_dp_bb_demo_0",
            &[
                "_dp_self".to_string(),
                "_dp_send_value".to_string(),
                "_dp_resume_exc".to_string(),
                "_dp_cell_captured".to_string(),
                "_dp_cell_x".to_string(),
                "_dp_cell__dp_pc".to_string(),
            ],
            "demo",
            "demo",
            &layout,
            false,
            false,
        );

        assert_eq!(block.label.as_str(), "_dp_bb_demo_factory");
        assert!(matches!(
            block.body.last(),
            Some(BlockPyStmt::Return(Some(_)))
        ));
    }

    #[test]
    fn lower_with_stmt_sequence_desugars_before_recursing() {
        let module = ruff_python_parser::parse_module(
            r#"
def f(ctx):
    with ctx():
        body()
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let ast::Stmt::With(with_stmt) = func.body.body[0].as_ref() else {
            panic!("expected with stmt");
        };

        let mut blocks = Vec::new();
        let mut saw_expanded = false;
        let entry = lower_with_stmt_sequence(
            with_stmt.clone(),
            &[],
            "cont".to_string(),
            Vec::new(),
            &mut blocks,
            None,
            &mut |expanded, cont_label, _blocks| {
                saw_expanded = true;
                assert_eq!(cont_label, "cont");
                assert!(expanded
                    .iter()
                    .any(|stmt| matches!(stmt.as_ref(), Stmt::Try(_))));
                "expanded_entry".to_string()
            },
        );

        assert!(saw_expanded);
        assert_eq!(entry, "expanded_entry");
    }

    #[test]
    fn lower_try_stmt_sequence_emits_legacy_try_jump() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    try:
        body()
    except ValueError:
        handle()
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let ast::Stmt::Try(try_stmt) = func.body.body[0].as_ref() else {
            panic!("expected try stmt");
        };

        let mut blocks = Vec::new();
        let mut next_label_id = 0usize;
        let try_plan = build_legacy_try_plan("demo", false, false, &mut next_label_id);
        let entry = lower_legacy_try_stmt_sequence(
            try_stmt.clone(),
            &[],
            "cont".to_string(),
            Vec::new(),
            &mut blocks,
            "_dp_bb_demo_legacy".to_string(),
            try_plan,
            &mut |_expanded, cont_label, _blocks| cont_label,
        );

        assert!(!entry.is_empty());
        assert!(blocks
            .iter()
            .any(|block| matches!(block.body.last(), Some(BlockPyStmt::LegacyTryJump(_)))));
    }

    #[test]
    fn expanded_stmt_helper_returns_expanded_entry_without_linear_prefix() {
        let mut blocks = Vec::new();
        let mut saw_expanded = false;
        let entry = lower_expanded_stmt_sequence(
            py_stmt!("pass"),
            &[],
            "cont".to_string(),
            Vec::new(),
            &mut blocks,
            None,
            &mut |expanded, cont_label, _blocks| {
                assert_eq!(expanded.len(), 1);
                assert_eq!(cont_label, "cont");
                saw_expanded = true;
                "expanded_entry".to_string()
            },
        );

        assert!(saw_expanded);
        assert_eq!(entry, "expanded_entry");
        assert!(blocks.is_empty());
    }

    #[test]
    fn expanded_stmt_helper_emits_linear_jump_prefix() {
        let mut blocks = Vec::new();
        let entry = lower_expanded_stmt_sequence(
            py_stmt!("pass"),
            &[],
            "cont".to_string(),
            vec![py_stmt!("x = 1")],
            &mut blocks,
            Some("prefix".to_string()),
            &mut |_expanded, _cont_label, _blocks| "expanded_entry".to_string(),
        );

        assert_eq!(entry, "prefix");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].label.as_str(), "prefix");
        assert!(matches!(
            blocks[0].body.last(),
            Some(BlockPyStmt::Jump(label)) if label.as_str() == "expanded_entry"
        ));
    }

    #[test]
    fn if_stmt_helper_lowers_both_branches_via_callback() {
        let mut blocks = Vec::new();
        let then_body = vec![Box::new(py_stmt!("x = 1"))];
        let else_body = vec![Box::new(py_stmt!("x = 2"))];
        let mut calls = Vec::new();

        let entry = lower_if_stmt_sequence(
            &mut blocks,
            "if_label".to_string(),
            vec![py_stmt!("prefix = 0")],
            py_expr!("flag"),
            &then_body,
            &else_body,
            "rest".to_string(),
            &mut |stmts, cont_label, _blocks| {
                calls.push((stmts.len(), cont_label.clone()));
                format!("branch_{}", calls.len())
            },
        );

        assert_eq!(entry, "if_label");
        assert_eq!(
            calls,
            vec![
                (then_body.len(), "rest".to_string()),
                (else_body.len(), "rest".to_string())
            ]
        );
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0].body.last(), Some(BlockPyStmt::If(_))));
    }

    #[test]
    fn sequence_jump_helper_emits_jump_block() {
        let mut blocks = Vec::new();
        let entry = emit_sequence_jump_block(
            &mut blocks,
            "jump_label".to_string(),
            vec![py_stmt!("prefix = 0")],
            "target".to_string(),
        );

        assert_eq!(entry, "jump_label");
        assert_eq!(blocks.len(), 1);
        assert!(matches!(
            blocks[0].body.last(),
            Some(BlockPyStmt::Jump(label)) if label.as_str() == "target"
        ));
    }

    #[test]
    fn sequence_return_helper_emits_return_block() {
        let mut blocks = Vec::new();
        let entry = emit_sequence_return_block(
            &mut blocks,
            "ret_label".to_string(),
            vec![py_stmt!("prefix = 0")],
            Some(py_expr!("value")),
        );

        assert_eq!(entry, "ret_label");
        assert_eq!(blocks.len(), 1);
        assert!(matches!(
            blocks[0].body.last(),
            Some(BlockPyStmt::Return(Some(_)))
        ));
    }

    #[test]
    fn sequence_raise_helper_emits_raise_block() {
        let mut blocks = Vec::new();
        let entry = emit_sequence_raise_block(
            &mut blocks,
            "raise_label".to_string(),
            vec![py_stmt!("prefix = 0")],
            BlockPyRaise {
                exc: Some(py_expr!("exc").into()),
            },
        );

        assert_eq!(entry, "raise_label");
        assert_eq!(blocks.len(), 1);
        assert!(matches!(
            blocks[0].body.last(),
            Some(BlockPyStmt::Raise(BlockPyRaise { exc: Some(_) }))
        ));
    }

    #[test]
    fn if_stmt_from_stmt_helper_lowers_remaining_and_branches() {
        let module = ruff_python_parser::parse_module(
            r#"
if flag:
    x = 1
else:
    x = 2
y = 3
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::If(if_stmt) = module.body[0].as_ref() else {
            panic!("expected if stmt");
        };
        let remaining = vec![module.body[1].clone()];
        let mut blocks = Vec::new();
        let mut calls = Vec::new();

        let entry = lower_if_stmt_sequence_from_stmt(
            if_stmt.clone(),
            &remaining,
            "cont".to_string(),
            vec![py_stmt!("prefix = 0")],
            &mut blocks,
            "if_label".to_string(),
            &mut |stmts, cont_label, _blocks| {
                calls.push((stmts.len(), cont_label.clone()));
                format!("branch_{}", calls.len())
            },
        );

        assert_eq!(entry, "if_label");
        assert_eq!(
            calls,
            vec![
                (remaining.len(), "cont".to_string()),
                (1, "branch_1".to_string()),
                (1, "branch_1".to_string())
            ]
        );
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0].body.last(), Some(BlockPyStmt::If(_))));
    }

    #[test]
    fn while_stmt_helper_lowers_loop_and_else_via_callbacks() {
        let mut blocks = Vec::new();
        let body = vec![Box::new(py_stmt!("x = 1"))];
        let else_body = vec![Box::new(py_stmt!("x = 2"))];
        let remaining = vec![Box::new(py_stmt!("x = 3"))];
        let mut sequence_calls = Vec::new();
        let mut loop_calls = Vec::new();

        let entry = lower_while_stmt_sequence(
            &mut blocks,
            "_dp_bb_loop_fn_0".to_string(),
            Some("_dp_bb_loop_fn_1".to_string()),
            vec![py_stmt!("prefix = 0")],
            py_expr!("flag"),
            &body,
            &else_body,
            &remaining,
            "cont".to_string(),
            &mut |stmts, cont_label, break_label, _blocks| {
                if let Some(break_label) = break_label {
                    loop_calls.push((stmts.len(), cont_label.clone(), break_label));
                    "loop_body".to_string()
                } else {
                    sequence_calls.push((stmts.len(), cont_label.clone()));
                    format!("seq_{}", sequence_calls.len())
                }
            },
        );

        assert_eq!(entry, "_dp_bb_loop_fn_1");
        assert_eq!(
            sequence_calls,
            vec![
                (remaining.len(), "cont".to_string()),
                (else_body.len(), "seq_1".to_string())
            ]
        );
        assert_eq!(
            loop_calls,
            vec![(
                body.len(),
                "_dp_bb_loop_fn_0".to_string(),
                "seq_1".to_string()
            )]
        );
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].label.as_str(), "_dp_bb_loop_fn_0");
        assert_eq!(blocks[1].label.as_str(), "_dp_bb_loop_fn_1");
    }

    #[test]
    fn while_stmt_from_stmt_helper_lowers_remaining_loop_and_else() {
        let module = ruff_python_parser::parse_module(
            r#"
while flag:
    x = 1
else:
    x = 2
y = 3
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::While(while_stmt) = module.body[0].as_ref() else {
            panic!("expected while stmt");
        };
        let remaining = vec![module.body[1].clone()];
        let mut blocks = Vec::new();
        let mut sequence_calls = Vec::new();
        let mut loop_calls = Vec::new();

        let entry = lower_while_stmt_sequence_from_stmt(
            while_stmt.clone(),
            &remaining,
            "cont".to_string(),
            vec![py_stmt!("prefix = 0")],
            &mut blocks,
            "_dp_bb_loop_fn_0".to_string(),
            Some("_dp_bb_loop_fn_1".to_string()),
            &mut |stmts, cont_label, break_label, _blocks| {
                if let Some(break_label) = break_label {
                    loop_calls.push((stmts.len(), cont_label.clone(), break_label));
                    "loop_body".to_string()
                } else {
                    sequence_calls.push((stmts.len(), cont_label.clone()));
                    format!("seq_{}", sequence_calls.len())
                }
            },
        );

        assert_eq!(entry, "_dp_bb_loop_fn_1");
        assert_eq!(
            sequence_calls,
            vec![
                (remaining.len(), "cont".to_string()),
                (1, "seq_1".to_string())
            ]
        );
        assert_eq!(
            loop_calls,
            vec![(
                1,
                "_dp_bb_loop_fn_0".to_string(),
                "seq_1".to_string()
            )]
        );
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].label.as_str(), "_dp_bb_loop_fn_0");
        assert_eq!(blocks[1].label.as_str(), "_dp_bb_loop_fn_1");
    }

    #[test]
    fn lowers_generator_yield_from_to_explicit_blockpy_dispatch() {
        let module = ruff_python_parser::parse_module(
            r#"
def gen(it):
    yield from it
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let blockpy = rewrite_ast_to_blockpy_module(&module, &function_identity(&func)).unwrap();
        let rendered = crate::basic_block::blockpy_module_to_string(&blockpy);
        assert!(rendered.contains("branch_table"));
        assert!(!rendered.contains("yield from it"));
    }

    #[test]
    fn lowers_async_generator_yield_to_explicit_blockpy_dispatch() {
        let module = ruff_python_parser::parse_module(
            r#"
async def agen(n):
    yield n
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let blockpy = rewrite_ast_to_blockpy_module(&module, &function_identity(&func)).unwrap();
        let rendered = crate::basic_block::blockpy_module_to_string(&blockpy);
        assert!(rendered.contains("branch_table"));
        assert!(!rendered.contains("yield n"));
    }

    #[test]
    fn lowers_module_prelude_statements() {
        let module = ruff_python_parser::parse_module(
            r#"
x = 1

def f():
    return 1
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[1].as_ref() else {
            panic!("expected function def");
        };
        let blockpy = rewrite_ast_to_blockpy_module(&module, &function_identity(&func)).unwrap();
        assert_eq!(blockpy.prelude.len(), 1);
        assert!(matches!(blockpy.prelude[0], BlockPyStmt::Assign(_)));
    }

    #[test]
    #[should_panic(expected = "Assert should be lowered before Ruff AST -> BlockPy conversion")]
    fn panics_if_assert_reaches_blockpy() {
        let module = ruff_python_parser::parse_module(
            r#"
def f(x):
    assert x
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let _ = rewrite_ast_to_blockpy_module(&module, &function_identity(&func)).unwrap();
    }

    #[test]
    #[should_panic(expected = "ClassDef should be lowered before Ruff AST -> BlockPy conversion")]
    fn panics_if_classdef_reaches_blockpy() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    class X:
        pass
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let _ = rewrite_ast_to_blockpy_module(&module, &function_identity(&func)).unwrap();
    }

    #[test]
    #[should_panic(expected = "AugAssign should be lowered before Ruff AST -> BlockPy conversion")]
    fn panics_if_augassign_reaches_blockpy() {
        let module = ruff_python_parser::parse_module(
            r#"
def f(x):
    x += 1
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let _ = rewrite_ast_to_blockpy_module(&module, &function_identity(&func)).unwrap();
    }

    #[test]
    #[should_panic(expected = "AnnAssign should be lowered before Ruff AST -> BlockPy conversion")]
    fn panics_if_annassign_reaches_blockpy() {
        let module = ruff_python_parser::parse_module(
            r#"
def f(x):
    y: int = x
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let _ = rewrite_ast_to_blockpy_module(&module, &function_identity(&func)).unwrap();
    }

    #[test]
    #[should_panic(expected = "TypeAlias should be lowered before Ruff AST -> BlockPy conversion")]
    fn panics_if_typealias_reaches_blockpy() {
        let module = ruff_python_parser::parse_module(
            r#"
type X = int

def f():
    return 1
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[1].as_ref() else {
            panic!("expected function def");
        };
        let _ = rewrite_ast_to_blockpy_module(&module, &function_identity(&func)).unwrap();
    }

    #[test]
    #[should_panic(expected = "Match should be lowered before Ruff AST -> BlockPy conversion")]
    fn panics_if_match_reaches_blockpy() {
        let module = ruff_python_parser::parse_module(
            r#"
def f(x):
    match x:
        case 1:
            return 1
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let _ = rewrite_ast_to_blockpy_module(&module, &function_identity(&func)).unwrap();
    }

    #[test]
    #[should_panic(expected = "Import should be lowered before Ruff AST -> BlockPy conversion")]
    fn panics_if_import_reaches_blockpy() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    import os
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let _ = rewrite_ast_to_blockpy_module(&module, &function_identity(&func)).unwrap();
    }

    #[test]
    #[should_panic(expected = "ImportFrom should be lowered before Ruff AST -> BlockPy conversion")]
    fn panics_if_importfrom_reaches_blockpy() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    from math import sqrt
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let _ = rewrite_ast_to_blockpy_module(&module, &function_identity(&func)).unwrap();
    }

    #[test]
    fn lowers_bare_raise_to_optional_blockpy_raise() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    raise
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let blockpy = rewrite_ast_to_blockpy_module(&module, &function_identity(&func)).unwrap();
        let raise_stmt = match &blockpy.functions[0].blocks[0].body[0] {
            BlockPyStmt::Raise(raise_stmt) => raise_stmt,
            other => panic!("expected BlockPy raise, got {other:?}"),
        };
        assert!(raise_stmt.exc.is_none());
    }

    #[test]
    #[should_panic(expected = "raise-from should be lowered before Ruff AST -> BlockPy conversion")]
    fn panics_if_raise_from_reaches_blockpy() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    raise E from cause
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let _ = rewrite_ast_to_blockpy_module(&module, &function_identity(&func)).unwrap();
    }

    #[test]
    #[should_panic(
        expected = "While should be lowered before Ruff AST -> BlockPy stmt-list conversion"
    )]
    fn panics_if_while_reaches_stmt_list_lowering() {
        let module = ruff_python_parser::parse_module("while x:\n    pass\n")
            .unwrap()
            .into_syntax()
            .body;
        let ast::Stmt::While(while_stmt) = module.body[0].as_ref() else {
            panic!("expected while stmt");
        };
        let mut out = Vec::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(
            &Stmt::While(while_stmt.clone()),
            &mut out,
            None,
            &mut next_label_id,
        )
        .unwrap();
    }
}
