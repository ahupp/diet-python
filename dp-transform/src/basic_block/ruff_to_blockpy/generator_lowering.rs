use super::{
    compat_block_from_blockpy, compat_if_jump_block, compat_jump_block_from_blockpy,
    compat_raise_block_from_blockpy_raise, compat_return_block_from_expr,
    lower_stmts_to_blockpy_stmts,
};
use crate::basic_block::ast_to_bb::sync_target_cells_stmts_shared as sync_target_cells_stmts;
use crate::basic_block::bb_ir::{
    BbGeneratorClosureInit, BbGeneratorClosureLayout, BbGeneratorClosureSlot,
};
use crate::basic_block::block_py::{
    BlockPyAssign, BlockPyBlock, BlockPyBranchTable, BlockPyExpr, BlockPyGeneratorInfo,
    BlockPyGeneratorYieldSite, BlockPyIf, BlockPyLabel, BlockPyLegacyTryJump, BlockPyRaise,
    BlockPyStmt,
};
use crate::basic_block::blockpy_to_bb::blockpy_stmt_to_stmt_for_analysis;
use crate::transform::scope::cell_name;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt};
use std::collections::{HashMap, HashSet};

fn blockpy_make_dp_tuple(items: Vec<Expr>) -> Expr {
    let Expr::Call(mut call) = py_expr!("__dp_tuple()") else {
        panic!("expected call expression for __dp_tuple");
    };
    call.arguments.args = items.into();
    Expr::Call(call)
}

fn sanitize_ident(raw: &str) -> String {
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

#[derive(Debug, Default)]
pub(crate) struct GeneratorDispatchInfo {
    pub done_block_label: Option<String>,
    pub invalid_block_label: Option<String>,
    pub generator_uncaught_label: Option<String>,
    pub generator_uncaught_exc_name: Option<String>,
    pub generator_uncaught_set_done_label: Option<String>,
    pub generator_uncaught_raise_label: Option<String>,
    pub generator_resume_entry_label: Option<String>,
    pub generator_resume_order: Vec<String>,
    pub generator_dispatch_only_labels: HashSet<String>,
    pub generator_throw_passthrough_labels: HashSet<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct GeneratorYieldSite {
    pub yield_label: String,
    pub resume_label: String,
}

pub(crate) struct BlockPyGeneratorLoweringResult {
    pub blocks: Vec<BlockPyBlock>,
    pub info: Option<BlockPyGeneratorInfo>,
}

pub(crate) fn lower_generator_blockpy_blocks_initial(
    fn_name: &str,
    mut blocks: Vec<BlockPyBlock>,
    closure_state: bool,
    cell_slots: &HashSet<String>,
    next_block_id: &mut usize,
) -> BlockPyGeneratorLoweringResult {
    let mut resume_order = Vec::new();
    let mut yield_sites = Vec::new();
    lower_generator_block_list(
        fn_name,
        &mut blocks,
        closure_state,
        cell_slots,
        next_block_id,
        &mut resume_order,
        &mut yield_sites,
    );
    let info = (!yield_sites.is_empty()).then(|| {
        let entry_label = blocks
            .first()
            .map(|block| block.label.as_str().to_string())
            .unwrap_or_else(|| format!("_dp_bb_{}_start", sanitize_ident(fn_name)));
        build_initial_generator_metadata(
            entry_label.as_str(),
            closure_state,
            &resume_order,
            &yield_sites,
        )
    });
    BlockPyGeneratorLoweringResult { blocks, info }
}

pub(crate) fn build_async_for_continue_entry(
    blocks: &mut Vec<BlockPyBlock>,
    fn_name: &str,
    iter_expr: Expr,
    tmp_name: &str,
    loop_check_label: &str,
    closure_state: bool,
    resume_order: &mut Vec<String>,
    yield_sites: &mut Vec<GeneratorYieldSite>,
    next_block_id: &mut usize,
) -> String {
    let await_value = py_expr!(
        "__dp_await_iter(__dp_anext_or_sentinel({iter:expr}))",
        iter = iter_expr,
    );
    let fetch_done_label = {
        let current = *next_block_id;
        *next_block_id += 1;
        format!("_dp_bb_{}_{}", sanitize_ident(fn_name), current)
    };
    let mut next_item = |item: GeneratorYieldFromPlanItem<'_>| match item {
        GeneratorYieldFromPlanItem::Temp(prefix) => {
            let current = *next_block_id;
            *next_block_id += 1;
            format!("_dp_{prefix}_{current}")
        }
        GeneratorYieldFromPlanItem::Label => {
            let current = *next_block_id;
            *next_block_id += 1;
            format!("_dp_bb_{}_{}", sanitize_ident(fn_name), current)
        }
    };
    let (fetch_entry_label, fetch_result_name) = lower_generator_yield_from_value(
        blocks,
        Vec::new(),
        await_value,
        fetch_done_label.clone(),
        closure_state,
        resume_order,
        yield_sites,
        &mut next_item,
    );
    let fetch_result_name =
        fetch_result_name.expect("async-for fetch lowering requires yielded result");
    blocks.push(compat_block_from_blockpy(
        fetch_done_label,
        vec![py_stmt!(
            "{tmp:id} = {value:id}",
            tmp = tmp_name,
            value = fetch_result_name.as_str(),
        )],
        BlockPyStmt::Jump(BlockPyLabel::from(loop_check_label.to_string())),
    ));
    fetch_entry_label
}

fn closure_backed_generator_init_expr(slot: &BbGeneratorClosureSlot) -> Expr {
    match slot.init {
        BbGeneratorClosureInit::InheritedCapture => {
            panic!("inherited captures do not allocate new cells in outer factories")
        }
        BbGeneratorClosureInit::Parameter => {
            py_expr!("{name:id}", name = slot.logical_name.as_str())
        }
        BbGeneratorClosureInit::DeletedSentinel => py_expr!("__dp_DELETED"),
        BbGeneratorClosureInit::RuntimePcZero => py_expr!("0"),
        BbGeneratorClosureInit::RuntimeNone => py_expr!("None"),
        BbGeneratorClosureInit::Deferred => py_expr!("None"),
    }
}

pub(crate) fn build_closure_backed_generator_factory_block(
    factory_label: &str,
    resume_label: &str,
    resume_state_order: &[String],
    function_name: &str,
    qualname: &str,
    layout: &BbGeneratorClosureLayout,
    is_coroutine: bool,
    is_async_generator: bool,
) -> BlockPyBlock {
    let hidden_name = "_dp_resume".to_string();
    let hidden_qualname = qualname.to_string();
    let mut body = Vec::new();

    for slot in layout
        .lifted_locals
        .iter()
        .chain(layout.runtime_cells.iter())
    {
        let stmt = py_stmt!(
            "{cell:id} = __dp_make_cell({init:expr})",
            cell = slot.storage_name.as_str(),
            init = closure_backed_generator_init_expr(slot),
        );
        let lowered = lower_stmts_to_blockpy_stmts(&[stmt])
            .unwrap_or_else(|err| panic!("failed to lower generator factory cell init: {err}"));
        body.extend(lowered);
    }

    let closure_names: Vec<String> = resume_state_order
        .iter()
        .filter(|state_name| {
            !matches!(
                state_name.as_str(),
                "_dp_self" | "_dp_send_value" | "_dp_resume_exc" | "_dp_transport_sent"
            )
        })
        .cloned()
        .collect();
    let closure_values = blockpy_make_dp_tuple(
        closure_names
            .iter()
            .map(|state_name| py_expr!("{name:id}", name = state_name.as_str()))
            .collect(),
    );

    let resume_entry = py_expr!(
        "__dp_def_hidden_resume_fn({resume:literal}, {name:literal}, {qualname:literal}, {state_order:expr}, {closure_names:expr}, {closure_values:expr}, __dp_globals(), __name__, async_gen={async_gen:expr})",
        resume = resume_label,
        name = hidden_name.as_str(),
        qualname = hidden_qualname.as_str(),
        state_order = blockpy_make_dp_tuple(
            resume_state_order
                .iter()
                .map(|state_name| py_expr!("{value:literal}", value = state_name.as_str()))
                .collect(),
        ),
        closure_names = blockpy_make_dp_tuple(
            closure_names
                .iter()
                .map(|state_name| py_expr!("{value:literal}", value = state_name.as_str()))
                .collect(),
        ),
        closure_values = closure_values,
        async_gen = if is_async_generator {
            py_expr!("True")
        } else {
            py_expr!("False")
        },
    );

    let generator_expr = if is_async_generator {
        py_expr!(
            "__dp_make_closure_async_generator({resume:expr}, {name:literal}, {qualname:literal})",
            resume = resume_entry,
            name = function_name,
            qualname = qualname,
        )
    } else {
        py_expr!(
            "__dp_make_closure_generator({resume:expr}, {name:literal}, {qualname:literal})",
            resume = resume_entry,
            name = function_name,
            qualname = qualname,
        )
    };

    let return_value = if is_coroutine {
        py_expr!(
            "__dp_make_coroutine_from_generator({gen:expr})",
            gen = generator_expr
        )
    } else {
        generator_expr
    };

    body.push(BlockPyStmt::Return(Some(return_value.into())));

    BlockPyBlock {
        label: factory_label.into(),
        body,
    }
}

pub(crate) fn build_initial_generator_metadata(
    dispatch_entry_label: &str,
    closure_state: bool,
    resume_order: &[String],
    yield_sites: &[GeneratorYieldSite],
) -> BlockPyGeneratorInfo {
    let mut resume_order = resume_order
        .iter()
        .cloned()
        .map(BlockPyLabel::from)
        .collect::<Vec<_>>();
    if !resume_order
        .iter()
        .any(|label| label.as_str() == dispatch_entry_label)
    {
        resume_order.insert(0, BlockPyLabel::from(dispatch_entry_label.to_string()));
    }
    BlockPyGeneratorInfo {
        closure_state,
        dispatch_entry_label: Some(BlockPyLabel::from(dispatch_entry_label.to_string())),
        resume_order,
        yield_sites: yield_sites
            .iter()
            .map(|site| BlockPyGeneratorYieldSite {
                yield_label: BlockPyLabel::from(site.yield_label.clone()),
                resume_label: BlockPyLabel::from(site.resume_label.clone()),
            })
            .collect(),
        done_block_label: None,
        invalid_block_label: None,
        uncaught_block_label: None,
        uncaught_set_done_label: None,
        uncaught_raise_label: None,
        uncaught_exc_name: None,
        dispatch_only_labels: Vec::new(),
        throw_passthrough_labels: Vec::new(),
    }
}

pub(crate) fn synthesize_generator_dispatch_metadata(
    blocks: &mut Vec<BlockPyBlock>,
    entry_label: &mut String,
    label_prefix: &str,
    is_async_generator_runtime: bool,
    is_closure_backed_generator_runtime: bool,
    uncaught_exc_name: String,
    resume_order: &[BlockPyLabel],
    yield_sites: &[BlockPyGeneratorYieldSite],
) -> BlockPyGeneratorInfo {
    let resume_order_raw = resume_order
        .iter()
        .map(|label| label.as_str().to_string())
        .collect::<Vec<_>>();
    let dispatch_info = synthesize_generator_dispatch(
        blocks,
        entry_label,
        label_prefix,
        is_async_generator_runtime,
        is_closure_backed_generator_runtime,
        uncaught_exc_name,
        &resume_order_raw,
    );
    BlockPyGeneratorInfo {
        closure_state: is_closure_backed_generator_runtime,
        dispatch_entry_label: dispatch_info
            .generator_resume_entry_label
            .as_ref()
            .map(|label| BlockPyLabel::from(label.clone())),
        resume_order: dispatch_info
            .generator_resume_order
            .iter()
            .cloned()
            .map(BlockPyLabel::from)
            .collect(),
        yield_sites: yield_sites.to_vec(),
        done_block_label: dispatch_info
            .done_block_label
            .as_ref()
            .map(|label| BlockPyLabel::from(label.clone())),
        invalid_block_label: dispatch_info
            .invalid_block_label
            .as_ref()
            .map(|label| BlockPyLabel::from(label.clone())),
        uncaught_block_label: dispatch_info
            .generator_uncaught_label
            .as_ref()
            .map(|label| BlockPyLabel::from(label.clone())),
        uncaught_set_done_label: dispatch_info
            .generator_uncaught_set_done_label
            .as_ref()
            .map(|label| BlockPyLabel::from(label.clone())),
        uncaught_raise_label: dispatch_info
            .generator_uncaught_raise_label
            .as_ref()
            .map(|label| BlockPyLabel::from(label.clone())),
        uncaught_exc_name: dispatch_info.generator_uncaught_exc_name.clone(),
        dispatch_only_labels: dispatch_info
            .generator_dispatch_only_labels
            .iter()
            .cloned()
            .map(BlockPyLabel::from)
            .collect(),
        throw_passthrough_labels: dispatch_info
            .generator_throw_passthrough_labels
            .iter()
            .cloned()
            .map(BlockPyLabel::from)
            .collect(),
    }
}

pub(crate) struct GeneratorYieldFromPlan {
    pub iter_name: String,
    pub yielded_name: String,
    pub sent_name: String,
    pub result_name: Option<String>,
    pub stop_name: String,
    pub exc_name: String,
    pub raise_name: String,
    pub close_name: String,
    pub throw_name: String,
    pub init_try_label: String,
    pub next_body_label: String,
    pub stop_except_label: String,
    pub stop_done_label: String,
    pub raise_stop_label: String,
    pub clear_done_label: String,
    pub clear_raise_label: String,
    pub resume_label: String,
    pub exc_dispatch_label: String,
    pub genexit_close_lookup_label: String,
    pub genexit_call_close_label: String,
    pub raise_exc_label: String,
    pub lookup_throw_label: String,
    pub throw_try_label: String,
    pub throw_body_label: String,
    pub send_try_label: String,
    pub send_dispatch_label: String,
    pub send_call_body_label: String,
    pub yield_label: String,
}

pub(crate) fn lower_generator_blockpy_blocks(
    fn_name: &str,
    blocks: Vec<BlockPyBlock>,
    closure_state: bool,
    is_async: bool,
    next_block_id: &mut usize,
) -> BlockPyGeneratorLoweringResult {
    let empty_cell_slots = HashSet::new();
    let BlockPyGeneratorLoweringResult {
        mut blocks,
        info: initial_info,
    } = lower_generator_blockpy_blocks_initial(
        fn_name,
        blocks,
        closure_state,
        &empty_cell_slots,
        next_block_id,
    );
    let mut info = None;
    if let Some(initial_generator_info) = initial_info {
        let mut entry_label = initial_generator_info
            .dispatch_entry_label
            .as_ref()
            .map(BlockPyLabel::as_str)
            .unwrap_or_else(|| {
                blocks
                    .first()
                    .map(|block| block.label.as_str())
                    .unwrap_or("_dp_bb_empty")
            })
            .to_string();
        let generator_info = synthesize_generator_dispatch_metadata(
            &mut blocks,
            &mut entry_label,
            sanitize_ident(fn_name).as_str(),
            is_async,
            closure_state,
            format!("_dp_uncaught_exc_{}", *next_block_id),
            &initial_generator_info.resume_order,
            &initial_generator_info.yield_sites,
        );
        let resume_pcs = generator_info
            .resume_order
            .iter()
            .enumerate()
            .map(|(pc, label)| (label.as_str().to_string(), pc))
            .collect::<Vec<_>>();
        let yield_sites = initial_generator_info
            .yield_sites
            .iter()
            .map(|site| GeneratorYieldSite {
                yield_label: site.yield_label.as_str().to_string(),
                resume_label: site.resume_label.as_str().to_string(),
            })
            .collect::<Vec<_>>();
        lower_generator_yield_terms_to_explicit_return_blockpy(
            &mut blocks,
            &HashMap::new(),
            &resume_pcs,
            &yield_sites,
            &[],
            is_async,
            closure_state,
        );
        info = Some(generator_info);
    }
    BlockPyGeneratorLoweringResult { blocks, info }
}

fn lower_generator_block_list(
    fn_name: &str,
    blocks: &mut Vec<BlockPyBlock>,
    closure_state: bool,
    cell_slots: &HashSet<String>,
    next_block_id: &mut usize,
    resume_order: &mut Vec<String>,
    yield_sites: &mut Vec<GeneratorYieldSite>,
) {
    let original = std::mem::take(blocks);
    let end_label = format!("_dp_bb_{}_end_{}", sanitize_ident(fn_name), *next_block_id);
    *next_block_id += 1;
    for idx in 0..original.len() {
        let mut block = original[idx].clone();
        lower_nested_generator_stmt_regions(
            fn_name,
            &mut block.body,
            closure_state,
            cell_slots,
            next_block_id,
            resume_order,
            yield_sites,
        );
        let next_label = original
            .get(idx + 1)
            .map(|next| next.label.as_str().to_string())
            .or_else(|| Some(end_label.clone()));
        lower_generator_block_body(
            fn_name,
            block,
            next_label,
            blocks,
            closure_state,
            cell_slots,
            next_block_id,
            resume_order,
            yield_sites,
        );
    }
    blocks.push(BlockPyBlock {
        label: BlockPyLabel::from(end_label),
        body: vec![BlockPyStmt::Return(None)],
    });
}

fn lower_nested_generator_stmt_regions(
    fn_name: &str,
    stmts: &mut [BlockPyStmt],
    closure_state: bool,
    cell_slots: &HashSet<String>,
    next_block_id: &mut usize,
    resume_order: &mut Vec<String>,
    yield_sites: &mut Vec<GeneratorYieldSite>,
) {
    for stmt in stmts {
        match stmt {
            BlockPyStmt::If(if_stmt) => {
                lower_generator_block_list(
                    fn_name,
                    &mut if_stmt.body,
                    closure_state,
                    cell_slots,
                    next_block_id,
                    resume_order,
                    yield_sites,
                );
                lower_generator_block_list(
                    fn_name,
                    &mut if_stmt.orelse,
                    closure_state,
                    cell_slots,
                    next_block_id,
                    resume_order,
                    yield_sites,
                );
            }
            BlockPyStmt::Try(try_stmt) => {
                lower_generator_block_list(
                    fn_name,
                    &mut try_stmt.body,
                    closure_state,
                    cell_slots,
                    next_block_id,
                    resume_order,
                    yield_sites,
                );
                for handler in &mut try_stmt.handlers {
                    lower_generator_block_list(
                        fn_name,
                        &mut handler.body,
                        closure_state,
                        cell_slots,
                        next_block_id,
                        resume_order,
                        yield_sites,
                    );
                }
                lower_generator_block_list(
                    fn_name,
                    &mut try_stmt.orelse,
                    closure_state,
                    cell_slots,
                    next_block_id,
                    resume_order,
                    yield_sites,
                );
                lower_generator_block_list(
                    fn_name,
                    &mut try_stmt.finalbody,
                    closure_state,
                    cell_slots,
                    next_block_id,
                    resume_order,
                    yield_sites,
                );
            }
            _ => {}
        }
    }
}

fn lower_generator_block_body(
    fn_name: &str,
    block: BlockPyBlock,
    cont_label: Option<String>,
    out: &mut Vec<BlockPyBlock>,
    closure_state: bool,
    cell_slots: &HashSet<String>,
    next_block_id: &mut usize,
    resume_order: &mut Vec<String>,
    yield_sites: &mut Vec<GeneratorYieldSite>,
) {
    let current_label = block.label.as_str().to_string();
    let stmts = block.body;
    let mut linear_blockpy = Vec::new();
    for (idx, stmt) in stmts.iter().enumerate() {
        let linear = linear_blockpy
            .iter()
            .filter_map(blockpy_stmt_to_stmt_for_analysis)
            .collect::<Vec<_>>();
        let mut lowered = Vec::new();
        let rest_entry = if blockpy_stmt_requires_generator_rest_entry(stmt) {
            if idx + 1 < stmts.len() {
                let rest_label = format!("_dp_bb_{}_{}", sanitize_ident(fn_name), *next_block_id);
                *next_block_id += 1;
                lower_generator_block_body(
                    fn_name,
                    BlockPyBlock {
                        label: BlockPyLabel::from(rest_label.clone()),
                        body: stmts[idx + 1..].to_vec(),
                    },
                    cont_label.clone(),
                    out,
                    closure_state,
                    cell_slots,
                    next_block_id,
                    resume_order,
                    yield_sites,
                );
                Some(rest_label)
            } else {
                cont_label.clone()
            }
        } else {
            None
        };
        if let Some(entry) = lower_generator_blockpy_stmt_in_sequence(
            stmt,
            linear,
            rest_entry,
            &mut lowered,
            closure_state,
            resume_order,
            yield_sites,
            next_block_id,
            fn_name,
            Some(cell_slots),
        ) {
            out.push(BlockPyBlock {
                label: BlockPyLabel::from(current_label),
                body: vec![BlockPyStmt::Jump(BlockPyLabel::from(entry))],
            });
            out.extend(lowered);
            return;
        }
        linear_blockpy.push(stmt.clone());
    }

    let mut body = linear_blockpy;
    if !body.last().is_some_and(|stmt| {
        matches!(
            stmt,
            BlockPyStmt::Jump(_)
                | BlockPyStmt::If(_)
                | BlockPyStmt::BranchTable(_)
                | BlockPyStmt::Raise(_)
                | BlockPyStmt::LegacyTryJump(_)
                | BlockPyStmt::Return(_)
        )
    }) {
        match cont_label {
            Some(label) => body.push(BlockPyStmt::Jump(BlockPyLabel::from(label))),
            None => body.push(BlockPyStmt::Return(None)),
        }
    }
    out.push(BlockPyBlock {
        label: BlockPyLabel::from(current_label),
        body,
    });
}

pub(crate) fn blockpy_stmt_requires_generator_rest_entry(stmt: &BlockPyStmt) -> bool {
    match stmt {
        BlockPyStmt::Expr(BlockPyExpr::Yield(_)) | BlockPyStmt::Expr(BlockPyExpr::YieldFrom(_)) => {
            true
        }
        BlockPyStmt::Assign(BlockPyAssign {
            value: BlockPyExpr::Yield(_),
            ..
        })
        | BlockPyStmt::Assign(BlockPyAssign {
            value: BlockPyExpr::YieldFrom(_),
            ..
        }) => true,
        _ => false,
    }
}

fn blockpy_assign_to_stmt(assign: &BlockPyAssign) -> ast::StmtAssign {
    ast::StmtAssign {
        node_index: ast::AtomicNodeIndex::default(),
        range: Default::default(),
        targets: vec![Expr::Name(assign.target.clone())],
        value: Box::new(assign.value.clone().into()),
    }
}

pub(crate) fn lower_generator_blockpy_stmt_in_sequence(
    stmt: &BlockPyStmt,
    linear: Vec<Stmt>,
    rest_entry: Option<String>,
    blocks: &mut Vec<BlockPyBlock>,
    closure_state: bool,
    resume_order: &mut Vec<String>,
    yield_sites: &mut Vec<GeneratorYieldSite>,
    next_block_id: &mut usize,
    fn_name: &str,
    cell_slots: Option<&HashSet<String>>,
) -> Option<String> {
    let fn_name_sanitized = sanitize_ident(fn_name);
    let mut next_item = |item: GeneratorYieldFromPlanItem<'_>| match item {
        GeneratorYieldFromPlanItem::Temp(prefix) => {
            let current = *next_block_id;
            *next_block_id += 1;
            format!("_dp_{prefix}_{current}")
        }
        GeneratorYieldFromPlanItem::Label => {
            let current = *next_block_id;
            *next_block_id += 1;
            format!("_dp_bb_{}_{}", fn_name_sanitized, current)
        }
    };
    let ctx = GeneratorLoweringCtx {
        blocks,
        closure_state,
        resume_order,
        yield_sites,
        next_item: &mut next_item,
    };
    match stmt {
        BlockPyStmt::Expr(BlockPyExpr::Yield(yield_expr)) => {
            let rest_entry =
                rest_entry.expect("generator expr lowering in stmt sequence requires a rest entry");
            let label = emit_generator_yield_suspend_blocks(
                ctx.blocks,
                linear,
                yield_expr.value.as_ref().map(|expr| *expr.clone()),
                rest_entry,
                (ctx.next_item)(GeneratorYieldFromPlanItem::Label),
                (ctx.next_item)(GeneratorYieldFromPlanItem::Label),
                (ctx.next_item)(GeneratorYieldFromPlanItem::Label),
                ctx.resume_order,
                ctx.yield_sites,
            );
            Some(label)
        }
        BlockPyStmt::Expr(BlockPyExpr::YieldFrom(yield_from_expr)) => {
            let rest_entry =
                rest_entry.expect("generator expr lowering in stmt sequence requires a rest entry");
            let label = (ctx.next_item)(GeneratorYieldFromPlanItem::Label);
            let plan = make_generator_yield_from_plan(&mut *ctx.next_item, false);
            let lowered = emit_generator_yield_from_expr_blocks(
                ctx.blocks,
                linear,
                *yield_from_expr.value.clone(),
                rest_entry,
                ctx.closure_state,
                ctx.resume_order,
                ctx.yield_sites,
                plan,
                label,
            );
            Some(lowered)
        }
        BlockPyStmt::Assign(
            assign_stmt @ BlockPyAssign {
                value: BlockPyExpr::Yield(yield_expr),
                ..
            },
        ) => {
            let rest_entry = rest_entry
                .expect("generator assign lowering in stmt sequence requires a rest entry");
            let resume_assign_label = (ctx.next_item)(GeneratorYieldFromPlanItem::Label);
            let resume_raise_label = (ctx.next_item)(GeneratorYieldFromPlanItem::Label);
            let resume_label = (ctx.next_item)(GeneratorYieldFromPlanItem::Label);
            let stmt_assign = blockpy_assign_to_stmt(assign_stmt);
            let label = emit_generator_assign_yield_suspend_blocks(
                ctx.blocks,
                linear,
                &stmt_assign,
                cell_slots.expect("generator assign lowering requires cell slots"),
                yield_expr.value.as_ref().map(|expr| *expr.clone()),
                rest_entry,
                resume_assign_label,
                GeneratorYieldLabels {
                    yield_label: (ctx.next_item)(GeneratorYieldFromPlanItem::Label),
                    resume_raise_label,
                    resume_dispatch_label: resume_label,
                },
                ctx.resume_order,
                ctx.yield_sites,
            );
            Some(label)
        }
        BlockPyStmt::Assign(
            assign_stmt @ BlockPyAssign {
                value: BlockPyExpr::YieldFrom(yield_from_expr),
                ..
            },
        ) => {
            let rest_entry = rest_entry
                .expect("generator assign lowering in stmt sequence requires a rest entry");
            let assign_result_label = (ctx.next_item)(GeneratorYieldFromPlanItem::Label);
            let label = (ctx.next_item)(GeneratorYieldFromPlanItem::Label);
            let plan = make_generator_yield_from_plan(&mut *ctx.next_item, true);
            let stmt_assign = blockpy_assign_to_stmt(assign_stmt);
            let lowered = emit_generator_yield_from_assign_blocks(
                ctx.blocks,
                linear,
                *yield_from_expr.value.clone(),
                &stmt_assign,
                cell_slots.expect("generator assign lowering requires cell slots"),
                rest_entry,
                ctx.closure_state,
                ctx.resume_order,
                ctx.yield_sites,
                plan,
                assign_result_label,
                label,
            );
            Some(lowered)
        }
        BlockPyStmt::Return(Some(BlockPyExpr::Yield(yield_expr))) => {
            let label = emit_generator_return_yield_suspend_blocks(
                ctx.blocks,
                linear,
                yield_expr.value.as_ref().map(|expr| *expr.clone()),
                (ctx.next_item)(GeneratorYieldFromPlanItem::Label),
                GeneratorYieldLabels {
                    yield_label: (ctx.next_item)(GeneratorYieldFromPlanItem::Label),
                    resume_raise_label: (ctx.next_item)(GeneratorYieldFromPlanItem::Label),
                    resume_dispatch_label: (ctx.next_item)(GeneratorYieldFromPlanItem::Label),
                },
                ctx.resume_order,
                ctx.yield_sites,
            );
            Some(label)
        }
        BlockPyStmt::Return(Some(BlockPyExpr::YieldFrom(yield_from_expr))) => {
            let return_label = (ctx.next_item)(GeneratorYieldFromPlanItem::Label);
            let label = (ctx.next_item)(GeneratorYieldFromPlanItem::Label);
            let plan = make_generator_yield_from_plan(&mut *ctx.next_item, true);
            let lowered = emit_generator_yield_from_return_blocks(
                ctx.blocks,
                linear,
                *yield_from_expr.value.clone(),
                ctx.closure_state,
                ctx.resume_order,
                ctx.yield_sites,
                plan,
                return_label,
                label,
            );
            Some(lowered)
        }
        _ => None,
    }
}

pub(crate) enum GeneratorYieldFromPlanItem<'a> {
    Temp(&'a str),
    Label,
}

pub(crate) fn make_generator_yield_from_plan<FNext>(
    mut next: FNext,
    capture_result: bool,
) -> GeneratorYieldFromPlan
where
    FNext: FnMut(GeneratorYieldFromPlanItem<'_>) -> String,
{
    GeneratorYieldFromPlan {
        iter_name: next(GeneratorYieldFromPlanItem::Temp("yield_from_iter")),
        yielded_name: next(GeneratorYieldFromPlanItem::Temp("yield_from_y")),
        sent_name: next(GeneratorYieldFromPlanItem::Temp("yield_from_sent")),
        result_name: if capture_result {
            Some(next(GeneratorYieldFromPlanItem::Temp("yield_from_result")))
        } else {
            None
        },
        stop_name: next(GeneratorYieldFromPlanItem::Temp("try_exc")),
        exc_name: next(GeneratorYieldFromPlanItem::Temp("yield_from_exc")),
        raise_name: next(GeneratorYieldFromPlanItem::Temp("yield_from_raise")),
        close_name: next(GeneratorYieldFromPlanItem::Temp("yield_from_close")),
        throw_name: next(GeneratorYieldFromPlanItem::Temp("yield_from_throw")),
        init_try_label: next(GeneratorYieldFromPlanItem::Label),
        next_body_label: next(GeneratorYieldFromPlanItem::Label),
        stop_except_label: next(GeneratorYieldFromPlanItem::Label),
        stop_done_label: next(GeneratorYieldFromPlanItem::Label),
        raise_stop_label: next(GeneratorYieldFromPlanItem::Label),
        clear_done_label: next(GeneratorYieldFromPlanItem::Label),
        clear_raise_label: next(GeneratorYieldFromPlanItem::Label),
        resume_label: next(GeneratorYieldFromPlanItem::Label),
        exc_dispatch_label: next(GeneratorYieldFromPlanItem::Label),
        genexit_close_lookup_label: next(GeneratorYieldFromPlanItem::Label),
        genexit_call_close_label: next(GeneratorYieldFromPlanItem::Label),
        raise_exc_label: next(GeneratorYieldFromPlanItem::Label),
        lookup_throw_label: next(GeneratorYieldFromPlanItem::Label),
        throw_try_label: next(GeneratorYieldFromPlanItem::Label),
        throw_body_label: next(GeneratorYieldFromPlanItem::Label),
        send_try_label: next(GeneratorYieldFromPlanItem::Label),
        send_dispatch_label: next(GeneratorYieldFromPlanItem::Label),
        send_call_body_label: next(GeneratorYieldFromPlanItem::Label),
        yield_label: next(GeneratorYieldFromPlanItem::Label),
    }
}

fn blockpy_raise_from_stmt(stmt: ruff_python_ast::StmtRaise) -> BlockPyRaise {
    BlockPyRaise {
        exc: stmt.exc.map(|expr| (*expr).into()),
    }
}

pub(crate) fn synthesize_generator_dispatch(
    blocks: &mut Vec<BlockPyBlock>,
    entry_label: &mut String,
    label_prefix: &str,
    is_async_generator_runtime: bool,
    is_closure_backed_generator_runtime: bool,
    uncaught_exc_name: String,
    resume_order: &[String],
) -> GeneratorDispatchInfo {
    let mut info = GeneratorDispatchInfo::default();
    let generator_pc_expr = if is_closure_backed_generator_runtime {
        py_expr!("__dp_load_cell(_dp_cell__dp_pc)")
    } else {
        py_expr!("__dp_getattr(_dp_self, \"_pc\")")
    };
    let done_label = format!("{label_prefix}_done");
    let invalid_label = format!("{label_prefix}_invalid");
    let uncaught_label = format!("{label_prefix}_uncaught");
    let invalid_msg = if is_async_generator_runtime {
        "invalid async generator pc: {}"
    } else {
        "invalid generator pc: {}"
    };
    let invalid_raise_stmt = match py_stmt!(
        "raise RuntimeError({msg:literal}.format({pc:expr}))",
        msg = invalid_msg,
        pc = generator_pc_expr.clone(),
    ) {
        Stmt::Raise(stmt) => stmt,
        _ => unreachable!("expected raise statement"),
    };
    blocks.insert(
        0,
        compat_return_block_from_expr(done_label.clone(), Vec::new(), None),
    );
    blocks.insert(
        1,
        compat_raise_block_from_blockpy_raise(
            invalid_label.clone(),
            Vec::new(),
            blockpy_raise_from_stmt(invalid_raise_stmt),
        ),
    );

    let uncaught_raise_stmt = match py_stmt!("raise {name:id}", name = uncaught_exc_name.as_str()) {
        Stmt::Raise(stmt) => stmt,
        _ => unreachable!("expected raise statement"),
    };
    let uncaught_helper_name = if is_async_generator_runtime {
        "raise_uncaught_async_generator_exception"
    } else {
        "raise_uncaught_generator_exception"
    };
    let uncaught_set_done_label = format!("{label_prefix}_uncaught_set_done");
    let uncaught_raise_label = format!("{label_prefix}_uncaught_raise");
    blocks.insert(
        2,
        compat_raise_block_from_blockpy_raise(
            uncaught_raise_label.clone(),
            Vec::new(),
            blockpy_raise_from_stmt(uncaught_raise_stmt),
        ),
    );
    blocks.insert(
        2,
        compat_jump_block_from_blockpy(
            uncaught_set_done_label.clone(),
            vec![
                if is_closure_backed_generator_runtime {
                    py_stmt!("__dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)")
                } else {
                    py_stmt!("__dp_setattr(_dp_self, \"_pc\", __dp_GEN_PC_DONE)")
                },
                py_stmt!(
                    "__dp_{helper:id}({exc:id})",
                    helper = uncaught_helper_name,
                    exc = uncaught_exc_name.as_str(),
                ),
            ],
            uncaught_raise_label.clone(),
        ),
    );
    blocks.insert(
        2,
        compat_if_jump_block(
            uncaught_label.clone(),
            Vec::new(),
            py_expr!(
                "__dp_ne({pc:expr}, __dp_GEN_PC_DONE)",
                pc = generator_pc_expr.clone()
            ),
            uncaught_set_done_label.clone(),
            uncaught_raise_label.clone(),
        ),
    );

    info.generator_throw_passthrough_labels
        .insert(uncaught_set_done_label.clone());
    info.generator_throw_passthrough_labels
        .insert(uncaught_raise_label.clone());
    info.done_block_label = Some(done_label.clone());
    info.invalid_block_label = Some(invalid_label.clone());
    info.generator_uncaught_label = Some(uncaught_label.clone());
    info.generator_uncaught_exc_name = Some(uncaught_exc_name.clone());
    info.generator_uncaught_set_done_label = Some(uncaught_set_done_label);
    info.generator_uncaught_raise_label = Some(uncaught_raise_label);

    info.generator_resume_entry_label = Some(entry_label.clone());
    info.generator_resume_order = resume_order.to_vec();

    let resume_throw_done_label = format!("{label_prefix}_dispatch_throw_done");
    let resume_throw_unstarted_label = format!("{label_prefix}_dispatch_throw_unstarted");
    info.generator_dispatch_only_labels
        .insert(resume_throw_done_label.clone());
    info.generator_dispatch_only_labels
        .insert(resume_throw_unstarted_label.clone());
    info.generator_throw_passthrough_labels
        .insert(resume_throw_done_label.clone());
    info.generator_throw_passthrough_labels
        .insert(resume_throw_unstarted_label.clone());
    let throw_resume_exc_stmt = match py_stmt!("raise _dp_resume_exc") {
        Stmt::Raise(stmt) => stmt,
        _ => unreachable!("expected raise statement"),
    };
    blocks.push(compat_raise_block_from_blockpy_raise(
        resume_throw_done_label.clone(),
        Vec::new(),
        blockpy_raise_from_stmt(throw_resume_exc_stmt.clone()),
    ));
    blocks.push(compat_raise_block_from_blockpy_raise(
        resume_throw_unstarted_label.clone(),
        Vec::new(),
        blockpy_raise_from_stmt(throw_resume_exc_stmt),
    ));

    let resume_send_label = format!("{label_prefix}_dispatch_send");
    let resume_throw_label = format!("{label_prefix}_dispatch_throw");
    let resume_dispatch_label = format!("{label_prefix}_dispatch");
    let resume_send_table_label = format!("{label_prefix}_dispatch_send_table");
    let resume_throw_table_label = format!("{label_prefix}_dispatch_throw_table");
    let resume_invalid_table_label = format!("{label_prefix}_dispatch_invalid");
    let resume_send_precheck_pc0_label = format!("{label_prefix}_dispatch_send_precheck_pc0");
    let resume_send_precheck_value_label = format!("{label_prefix}_dispatch_send_precheck_value");
    let resume_send_precheck_transport_label =
        format!("{label_prefix}_dispatch_send_precheck_transport");
    let resume_send_transport_error_label = format!("{label_prefix}_dispatch_send_transport_error");
    info.generator_dispatch_only_labels.extend([
        resume_send_label.clone(),
        resume_throw_label.clone(),
        resume_dispatch_label.clone(),
        resume_send_table_label.clone(),
        resume_throw_table_label.clone(),
        resume_invalid_table_label.clone(),
    ]);
    if is_async_generator_runtime {
        info.generator_dispatch_only_labels.extend([
            resume_send_precheck_pc0_label.clone(),
            resume_send_precheck_value_label.clone(),
            resume_send_precheck_transport_label.clone(),
            resume_send_transport_error_label.clone(),
        ]);
    }

    let mut send_table_targets = Vec::with_capacity(resume_order.len());
    let mut throw_table_targets = Vec::with_capacity(resume_order.len());
    for (pc, resume_target) in resume_order.iter().enumerate() {
        let send_dispatch_target_label = format!("{label_prefix}_dispatch_send_target_{pc}");
        info.generator_dispatch_only_labels
            .insert(send_dispatch_target_label.clone());
        blocks.push(compat_block_from_blockpy(
            send_dispatch_target_label.clone(),
            Vec::new(),
            BlockPyStmt::Jump(BlockPyLabel::from(resume_target.clone())),
        ));
        send_table_targets.push(send_dispatch_target_label);

        let throw_dispatch_target_label = format!("{label_prefix}_dispatch_throw_target_{pc}");
        info.generator_dispatch_only_labels
            .insert(throw_dispatch_target_label.clone());
        let throw_target = if pc == 0 {
            resume_throw_unstarted_label.clone()
        } else {
            resume_target.clone()
        };
        blocks.push(compat_block_from_blockpy(
            throw_dispatch_target_label.clone(),
            Vec::new(),
            BlockPyStmt::Jump(BlockPyLabel::from(throw_target)),
        ));
        throw_table_targets.push(throw_dispatch_target_label);
    }
    blocks.push(compat_block_from_blockpy(
        resume_invalid_table_label.clone(),
        Vec::new(),
        BlockPyStmt::Jump(BlockPyLabel::from(invalid_label)),
    ));

    blocks.push(compat_block_from_blockpy(
        resume_send_table_label.clone(),
        Vec::new(),
        BlockPyStmt::BranchTable(BlockPyBranchTable {
            index: generator_pc_expr.clone().into(),
            targets: send_table_targets
                .into_iter()
                .map(BlockPyLabel::from)
                .collect(),
            default_label: BlockPyLabel::from(resume_invalid_table_label.clone()),
        }),
    ));
    blocks.push(compat_block_from_blockpy(
        resume_throw_table_label.clone(),
        Vec::new(),
        BlockPyStmt::BranchTable(BlockPyBranchTable {
            index: generator_pc_expr.clone().into(),
            targets: throw_table_targets
                .into_iter()
                .map(BlockPyLabel::from)
                .collect(),
            default_label: BlockPyLabel::from(resume_invalid_table_label),
        }),
    ));
    if is_async_generator_runtime {
        let transport_error_raise_stmt = match py_stmt!(
            "raise TypeError(\"can't send non-None value to a just-started async generator\")"
        ) {
            Stmt::Raise(stmt) => stmt,
            _ => unreachable!("expected raise statement"),
        };
        blocks.push(compat_block_from_blockpy(
            resume_send_transport_error_label.clone(),
            Vec::new(),
            BlockPyStmt::Raise(blockpy_raise_from_stmt(transport_error_raise_stmt)),
        ));
        blocks.push(compat_if_jump_block(
            resume_send_precheck_transport_label.clone(),
            Vec::new(),
            py_expr!("__dp_is_not(_dp_transport_sent, None)"),
            resume_send_transport_error_label,
            resume_send_table_label.clone(),
        ));
        blocks.push(compat_if_jump_block(
            resume_send_precheck_value_label.clone(),
            Vec::new(),
            py_expr!("__dp_is_(_dp_send_value, None)"),
            resume_send_precheck_transport_label,
            resume_send_table_label.clone(),
        ));
        blocks.push(compat_if_jump_block(
            resume_send_precheck_pc0_label.clone(),
            Vec::new(),
            py_expr!("__dp_eq({pc:expr}, 0)", pc = generator_pc_expr.clone()),
            resume_send_precheck_value_label,
            resume_send_table_label.clone(),
        ));
    }
    blocks.push(compat_if_jump_block(
        resume_send_label.clone(),
        Vec::new(),
        py_expr!(
            "__dp_eq({pc:expr}, __dp_GEN_PC_DONE)",
            pc = generator_pc_expr.clone()
        ),
        done_label,
        if is_async_generator_runtime {
            resume_send_precheck_pc0_label
        } else {
            resume_send_table_label
        },
    ));
    blocks.push(compat_if_jump_block(
        resume_throw_label.clone(),
        Vec::new(),
        py_expr!(
            "__dp_eq({pc:expr}, __dp_GEN_PC_DONE)",
            pc = generator_pc_expr
        ),
        resume_throw_done_label,
        resume_throw_table_label,
    ));
    blocks.push(compat_if_jump_block(
        resume_dispatch_label.clone(),
        Vec::new(),
        py_expr!("__dp_is_(_dp_resume_exc, None)"),
        resume_send_label,
        resume_throw_label,
    ));
    *entry_label = resume_dispatch_label;

    info
}

fn lower_generated_stmts_to_blockpy(stmts: Vec<Stmt>) -> Vec<BlockPyStmt> {
    lower_stmts_to_blockpy_stmts(&stmts)
        .unwrap_or_else(|err| panic!("failed to convert generated stmt to BlockPy: {err}"))
}

pub(crate) fn lower_generator_yield_terms_to_explicit_return_blockpy(
    blocks: &mut [BlockPyBlock],
    block_params: &HashMap<String, Vec<String>>,
    resume_pcs: &[(String, usize)],
    yield_sites: &[GeneratorYieldSite],
    cleanup_cells: &[String],
    is_async: bool,
    closure_state: bool,
) {
    let resume_pc_by_label = resume_pcs
        .iter()
        .cloned()
        .collect::<HashMap<String, usize>>();
    let yield_resume_by_label = yield_sites
        .iter()
        .map(|site| (site.yield_label.clone(), site.resume_label.clone()))
        .collect::<HashMap<String, String>>();

    for block in blocks.iter_mut() {
        let Some(last_stmt) = block.body.last().cloned() else {
            continue;
        };
        match last_stmt {
            BlockPyStmt::Return(yielded_value)
                if yield_resume_by_label.contains_key(block.label.as_str()) =>
            {
                let resume_label = yield_resume_by_label
                    .get(block.label.as_str())
                    .expect("yield site should have a resume label");
                let next_pc = *resume_pc_by_label
                    .get(resume_label.as_str())
                    .unwrap_or_else(|| {
                        panic!("missing resume pc for label: {}", resume_label.as_str())
                    });
                let mut injected = if closure_state {
                    lower_generated_stmts_to_blockpy(vec![py_stmt!(
                        "__dp_store_cell(_dp_cell__dp_pc, {next_pc:literal})",
                        next_pc = next_pc as i64,
                    )])
                } else {
                    lower_generated_stmts_to_blockpy(vec![py_stmt!(
                        "__dp_setattr(_dp_self, \"_pc\", {next_pc:literal})",
                        next_pc = next_pc as i64,
                    )])
                };
                if !closure_state {
                    let next_state_names = block_params
                        .get(resume_label.as_str())
                        .cloned()
                        .unwrap_or_default();
                    for name in next_state_names {
                        if matches!(
                            name.as_str(),
                            "_dp_self" | "_dp_send_value" | "_dp_resume_exc" | "_dp_transport_sent"
                        ) {
                            continue;
                        }
                        injected.extend(lower_generated_stmts_to_blockpy(vec![py_stmt!(
                            "__dp_store_local(_dp_self, {name:literal}, {value:id})",
                            name = name.as_str(),
                            value = name.as_str(),
                        )]));
                    }
                }
                block.body.pop();
                block.body.extend(injected);
                block.body.push(BlockPyStmt::Return(yielded_value));
            }
            BlockPyStmt::Return(value) => {
                let mut injected = if closure_state {
                    lower_generated_stmts_to_blockpy(vec![py_stmt!(
                        "__dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)"
                    )])
                } else {
                    lower_generated_stmts_to_blockpy(vec![py_stmt!(
                        "__dp_setattr(_dp_self, \"_pc\", __dp_GEN_PC_DONE)"
                    )])
                };
                if closure_state {
                    for cell in cleanup_cells {
                        injected.extend(lower_generated_stmts_to_blockpy(vec![py_stmt!(
                            "__dp_store_cell({cell:id}, __dp_DELETED)",
                            cell = cell.as_str(),
                        )]));
                    }
                }
                block.body.pop();
                block.body.extend(injected);
                let raise_stmt = if is_async {
                    match py_stmt!("raise StopAsyncIteration()") {
                        Stmt::Raise(stmt) => stmt,
                        _ => unreachable!("expected raise statement"),
                    }
                } else if let Some(value) = value.clone() {
                    match py_stmt!("raise StopIteration({value:expr})", value = value.to_expr()) {
                        Stmt::Raise(stmt) => stmt,
                        _ => unreachable!("expected raise statement"),
                    }
                } else {
                    match py_stmt!("raise StopIteration()") {
                        Stmt::Raise(stmt) => stmt,
                        _ => unreachable!("expected raise statement"),
                    }
                };
                block
                    .body
                    .push(BlockPyStmt::Raise(blockpy_raise_from_stmt(raise_stmt)));
            }
            _ => {}
        }
    }
}

pub(crate) fn emit_generator_yield_suspend_blocks(
    blocks: &mut Vec<BlockPyBlock>,
    linear: Vec<Stmt>,
    yield_value: Option<Expr>,
    resume_success_label: String,
    yield_label: String,
    resume_raise_label: String,
    resume_dispatch_label: String,
    resume_order: &mut Vec<String>,
    yield_sites: &mut Vec<GeneratorYieldSite>,
) -> String {
    blocks.push(compat_block_from_blockpy(
        resume_raise_label.clone(),
        Vec::new(),
        BlockPyStmt::Raise(BlockPyRaise {
            exc: Some(py_expr!("{name:id}", name = "_dp_resume_exc").into()),
        }),
    ));
    blocks.push(compat_block_from_blockpy(
        resume_dispatch_label.clone(),
        Vec::new(),
        BlockPyStmt::If(BlockPyIf {
            test: py_expr!("__dp_is_not(_dp_resume_exc, None)").into(),
            body: vec![BlockPyBlock {
                label: BlockPyLabel::from(format!("{resume_dispatch_label}_true")),
                body: vec![BlockPyStmt::Jump(BlockPyLabel::from(resume_raise_label))],
            }],
            orelse: vec![BlockPyBlock {
                label: BlockPyLabel::from(format!("{resume_dispatch_label}_false")),
                body: vec![BlockPyStmt::Jump(BlockPyLabel::from(resume_success_label))],
            }],
        }),
    ));
    if !resume_order
        .iter()
        .any(|label| label == &resume_dispatch_label)
    {
        resume_order.push(resume_dispatch_label.clone());
    }
    yield_sites.push(GeneratorYieldSite {
        yield_label: yield_label.clone(),
        resume_label: resume_dispatch_label.clone(),
    });
    blocks.push(compat_return_block_from_expr(
        yield_label.clone(),
        linear,
        yield_value,
    ));
    yield_label
}

pub(crate) struct GeneratorYieldLabels {
    pub yield_label: String,
    pub resume_raise_label: String,
    pub resume_dispatch_label: String,
}

pub(crate) fn emit_generator_assign_yield_suspend_blocks(
    blocks: &mut Vec<BlockPyBlock>,
    linear: Vec<Stmt>,
    assign_stmt: &ruff_python_ast::StmtAssign,
    cell_slots: &HashSet<String>,
    yield_value: Option<Expr>,
    rest_entry: String,
    resume_assign_label: String,
    labels: GeneratorYieldLabels,
    resume_order: &mut Vec<String>,
    yield_sites: &mut Vec<GeneratorYieldSite>,
) -> String {
    let mut resume_assign = assign_stmt.clone();
    resume_assign.value = Box::new(py_expr!("{sent:id}", sent = "_dp_send_value"));
    let mut resume_body = vec![Stmt::Assign(resume_assign.clone())];
    for target in &resume_assign.targets {
        resume_body.extend(sync_target_cells_stmts(target, cell_slots));
    }
    blocks.push(compat_jump_block_from_blockpy(
        resume_assign_label.clone(),
        resume_body,
        rest_entry,
    ));
    emit_generator_yield_suspend_blocks(
        blocks,
        linear,
        yield_value,
        resume_assign_label,
        labels.yield_label,
        labels.resume_raise_label,
        labels.resume_dispatch_label,
        resume_order,
        yield_sites,
    )
}

pub(crate) fn emit_generator_return_yield_suspend_blocks(
    blocks: &mut Vec<BlockPyBlock>,
    linear: Vec<Stmt>,
    yield_value: Option<Expr>,
    resume_return_label: String,
    labels: GeneratorYieldLabels,
    resume_order: &mut Vec<String>,
    yield_sites: &mut Vec<GeneratorYieldSite>,
) -> String {
    blocks.push(compat_return_block_from_expr(
        resume_return_label.clone(),
        Vec::new(),
        Some(py_expr!("{sent:id}", sent = "_dp_send_value")),
    ));
    emit_generator_yield_suspend_blocks(
        blocks,
        linear,
        yield_value,
        resume_return_label,
        labels.yield_label,
        labels.resume_raise_label,
        labels.resume_dispatch_label,
        resume_order,
        yield_sites,
    )
}

pub(crate) fn emit_yield_from_blocks(
    value: Expr,
    after_label: String,
    blocks: &mut Vec<BlockPyBlock>,
    closure_state: bool,
    resume_order: &mut Vec<String>,
    yield_sites: &mut Vec<GeneratorYieldSite>,
    plan: GeneratorYieldFromPlan,
) -> (String, Option<String>) {
    let yieldfrom_cell_name = cell_name("_dp_yieldfrom");
    let yieldfrom_load_expr = if closure_state {
        py_expr!(
            "__dp_load_cell({cell:id})",
            cell = yieldfrom_cell_name.as_str(),
        )
    } else {
        py_expr!("__dp_load_local(_dp_self, \"_dp_yieldfrom\")")
    };
    let yieldfrom_load_raw_expr = if closure_state {
        py_expr!(
            "__dp_load_cell({cell:id})",
            cell = yieldfrom_cell_name.as_str(),
        )
    } else {
        py_expr!("__dp_load_local_raw(_dp_self, \"_dp_yieldfrom\")")
    };
    let yieldfrom_store_iter_stmt = if closure_state {
        py_stmt!(
            "__dp_store_cell({cell:id}, {iter_name:id})",
            cell = yieldfrom_cell_name.as_str(),
            iter_name = plan.iter_name.as_str(),
        )
    } else {
        py_stmt!(
            "__dp_store_local(_dp_self, \"_dp_yieldfrom\", {iter_name:id})",
            iter_name = plan.iter_name.as_str(),
        )
    };
    let yieldfrom_clear_stmt = if closure_state {
        py_stmt!(
            "__dp_store_cell({cell:id}, None)",
            cell = yieldfrom_cell_name.as_str(),
        )
    } else {
        py_stmt!("__dp_store_local(_dp_self, \"_dp_yieldfrom\", None)")
    };

    blocks.push(compat_block_from_blockpy(
        plan.init_try_label.clone(),
        vec![
            py_stmt!(
                "{iter_name:id} = iter({iter_expr:expr})",
                iter_name = plan.iter_name.as_str(),
                iter_expr = value,
            ),
            yieldfrom_store_iter_stmt,
        ],
        BlockPyStmt::LegacyTryJump(BlockPyLegacyTryJump {
            body_label: BlockPyLabel::from(plan.next_body_label.clone()),
            except_label: BlockPyLabel::from(plan.stop_except_label.clone()),
            except_exc_name: Some(plan.stop_name.clone()),
            body_region_labels: vec![BlockPyLabel::from(plan.next_body_label.clone())],
            except_region_labels: vec![
                BlockPyLabel::from(plan.stop_except_label.clone()),
                BlockPyLabel::from(plan.stop_done_label.clone()),
                BlockPyLabel::from(plan.raise_stop_label.clone()),
            ],
            finally_label: None,
            finally_exc_name: None,
            finally_region_labels: Vec::new(),
            finally_fallthrough_label: None,
        }),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.next_body_label.clone(),
        vec![py_stmt!(
            "{yielded:id} = next({iter_expr:expr})",
            yielded = plan.yielded_name.as_str(),
            iter_expr = yieldfrom_load_expr.clone(),
        )],
        BlockPyStmt::Jump(BlockPyLabel::from(plan.yield_label.clone())),
    ));
    blocks.push(compat_if_jump_block(
        plan.stop_except_label.clone(),
        Vec::new(),
        py_expr!(
            "__dp_exception_matches({stop:id}, StopIteration)",
            stop = plan.stop_name.as_str(),
        ),
        plan.stop_done_label.clone(),
        plan.raise_stop_label.clone(),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.stop_done_label.clone(),
        if let Some(result_name) = plan.result_name.as_ref() {
            vec![py_stmt!(
                "{result:id} = {stop:id}.value",
                result = result_name.as_str(),
                stop = plan.stop_name.as_str(),
            )]
        } else {
            Vec::new()
        },
        BlockPyStmt::Jump(BlockPyLabel::from(plan.clear_done_label.clone())),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.clear_done_label,
        vec![yieldfrom_clear_stmt.clone()],
        BlockPyStmt::Jump(BlockPyLabel::from(after_label)),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.raise_stop_label.clone(),
        vec![py_stmt!(
            "{raise:id} = {stop:id}",
            raise = plan.raise_name.as_str(),
            stop = plan.stop_name.as_str(),
        )],
        BlockPyStmt::Jump(BlockPyLabel::from(plan.clear_raise_label.clone())),
    ));
    if !resume_order.iter().any(|label| label == &plan.resume_label) {
        resume_order.push(plan.resume_label.clone());
    }
    yield_sites.push(GeneratorYieldSite {
        yield_label: plan.yield_label.clone(),
        resume_label: plan.resume_label.clone(),
    });
    blocks.push(compat_block_from_blockpy(
        plan.yield_label.clone(),
        Vec::new(),
        BlockPyStmt::Return(Some(
            py_expr!("{yielded:id}", yielded = plan.yielded_name.as_str(),).into(),
        )),
    ));
    let yield_label = plan.yield_label.clone();
    blocks.push(compat_if_jump_block(
        plan.resume_label,
        vec![
            py_stmt!(
                "{sent:id} = {resume:id}",
                sent = plan.sent_name.as_str(),
                resume = "_dp_send_value",
            ),
            py_stmt!(
                "{exc:id} = {resume:id}",
                exc = plan.exc_name.as_str(),
                resume = "_dp_resume_exc",
            ),
            py_stmt!("{resume:id} = None", resume = "_dp_resume_exc",),
        ],
        py_expr!("__dp_is_not({exc:id}, None)", exc = plan.exc_name.as_str()).into(),
        plan.exc_dispatch_label.clone(),
        plan.send_try_label.clone(),
    ));
    blocks.push(compat_if_jump_block(
        plan.exc_dispatch_label,
        Vec::new(),
        py_expr!(
            "__dp_exception_matches({exc:id}, GeneratorExit)",
            exc = plan.exc_name.as_str(),
        )
        .into(),
        plan.genexit_close_lookup_label.clone(),
        plan.lookup_throw_label.clone(),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.genexit_close_lookup_label.clone(),
        vec![py_stmt!(
            "{close:id} = getattr({iter_expr:expr}, \"close\", None)",
            close = plan.close_name.as_str(),
            iter_expr = yieldfrom_load_raw_expr.clone(),
        )],
        BlockPyStmt::If(BlockPyIf {
            test: py_expr!(
                "__dp_is_not({close:id}, None)",
                close = plan.close_name.as_str()
            )
            .into(),
            body: vec![BlockPyBlock {
                label: BlockPyLabel::from(format!("{}_true", plan.genexit_close_lookup_label)),
                body: vec![BlockPyStmt::Jump(BlockPyLabel::from(
                    plan.genexit_call_close_label.clone(),
                ))],
            }],
            orelse: vec![BlockPyBlock {
                label: BlockPyLabel::from(format!("{}_false", plan.genexit_close_lookup_label)),
                body: vec![BlockPyStmt::Jump(BlockPyLabel::from(
                    plan.raise_exc_label.clone(),
                ))],
            }],
        }),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.genexit_call_close_label,
        vec![py_stmt!("{close:id}()", close = plan.close_name.as_str())],
        BlockPyStmt::Jump(BlockPyLabel::from(plan.raise_exc_label.clone())),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.raise_exc_label.clone(),
        vec![py_stmt!(
            "{raise:id} = {exc:id}",
            raise = plan.raise_name.as_str(),
            exc = plan.exc_name.as_str(),
        )],
        BlockPyStmt::Jump(BlockPyLabel::from(plan.clear_raise_label.clone())),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.clear_raise_label,
        vec![yieldfrom_clear_stmt],
        BlockPyStmt::Raise(BlockPyRaise {
            exc: Some(py_expr!("{name:id}", name = plan.raise_name.as_str()).into()),
        }),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.lookup_throw_label.clone(),
        vec![py_stmt!(
            "{throw:id} = getattr({iter_expr:expr}, \"throw\", None)",
            throw = plan.throw_name.as_str(),
            iter_expr = yieldfrom_load_raw_expr,
        )],
        BlockPyStmt::If(BlockPyIf {
            test: py_expr!(
                "__dp_is_({throw:id}, None)",
                throw = plan.throw_name.as_str()
            )
            .into(),
            body: vec![BlockPyBlock {
                label: BlockPyLabel::from(format!("{}_true", plan.lookup_throw_label)),
                body: vec![BlockPyStmt::Jump(BlockPyLabel::from(plan.raise_exc_label))],
            }],
            orelse: vec![BlockPyBlock {
                label: BlockPyLabel::from(format!("{}_false", plan.lookup_throw_label)),
                body: vec![BlockPyStmt::Jump(BlockPyLabel::from(
                    plan.throw_try_label.clone(),
                ))],
            }],
        }),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.throw_try_label,
        Vec::new(),
        BlockPyStmt::LegacyTryJump(BlockPyLegacyTryJump {
            body_label: BlockPyLabel::from(plan.throw_body_label.clone()),
            except_label: BlockPyLabel::from(plan.stop_except_label.clone()),
            except_exc_name: Some(plan.stop_name.clone()),
            body_region_labels: vec![BlockPyLabel::from(plan.throw_body_label.clone())],
            except_region_labels: vec![
                BlockPyLabel::from(plan.stop_except_label.clone()),
                BlockPyLabel::from(plan.stop_done_label.clone()),
                BlockPyLabel::from(plan.raise_stop_label.clone()),
            ],
            finally_label: None,
            finally_exc_name: None,
            finally_region_labels: Vec::new(),
            finally_fallthrough_label: None,
        }),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.throw_body_label,
        vec![py_stmt!(
            "{yielded:id} = {throw:id}({exc:id})",
            yielded = plan.yielded_name.as_str(),
            throw = plan.throw_name.as_str(),
            exc = plan.exc_name.as_str(),
        )],
        BlockPyStmt::Jump(BlockPyLabel::from(yield_label.clone())),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.send_try_label,
        Vec::new(),
        BlockPyStmt::LegacyTryJump(BlockPyLegacyTryJump {
            body_label: BlockPyLabel::from(plan.send_dispatch_label.clone()),
            except_label: BlockPyLabel::from(plan.stop_except_label.clone()),
            except_exc_name: Some(plan.stop_name.clone()),
            body_region_labels: vec![
                BlockPyLabel::from(plan.send_dispatch_label.clone()),
                BlockPyLabel::from(plan.next_body_label.clone()),
                BlockPyLabel::from(plan.send_call_body_label.clone()),
            ],
            except_region_labels: vec![
                BlockPyLabel::from(plan.stop_except_label.clone()),
                BlockPyLabel::from(plan.stop_done_label.clone()),
                BlockPyLabel::from(plan.raise_stop_label.clone()),
            ],
            finally_label: None,
            finally_exc_name: None,
            finally_region_labels: Vec::new(),
            finally_fallthrough_label: None,
        }),
    ));
    blocks.push(compat_if_jump_block(
        plan.send_dispatch_label,
        Vec::new(),
        py_expr!("__dp_is_({sent:id}, None)", sent = plan.sent_name.as_str()).into(),
        plan.next_body_label,
        plan.send_call_body_label.clone(),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.send_call_body_label,
        vec![py_stmt!(
            "{yielded:id} = {iter_expr:expr}.send({sent:id})",
            yielded = plan.yielded_name.as_str(),
            iter_expr = yieldfrom_load_expr,
            sent = plan.sent_name.as_str(),
        )],
        BlockPyStmt::Jump(BlockPyLabel::from(yield_label)),
    ));
    (plan.init_try_label, plan.result_name)
}

pub(crate) fn emit_generator_yield_from_expr_blocks(
    blocks: &mut Vec<BlockPyBlock>,
    linear: Vec<Stmt>,
    value: Expr,
    after_label: String,
    closure_state: bool,
    resume_order: &mut Vec<String>,
    yield_sites: &mut Vec<GeneratorYieldSite>,
    plan: GeneratorYieldFromPlan,
    jump_to_yield_from_label: String,
) -> String {
    let (yield_from_entry, _result_name) = emit_yield_from_blocks(
        value,
        after_label,
        blocks,
        closure_state,
        resume_order,
        yield_sites,
        plan,
    );
    blocks.push(compat_block_from_blockpy(
        jump_to_yield_from_label.clone(),
        linear,
        BlockPyStmt::Jump(BlockPyLabel::from(yield_from_entry)),
    ));
    jump_to_yield_from_label
}

pub(crate) fn lower_generator_yield_from_value(
    blocks: &mut Vec<BlockPyBlock>,
    linear: Vec<Stmt>,
    value: Expr,
    after_label: String,
    closure_state: bool,
    resume_order: &mut Vec<String>,
    yield_sites: &mut Vec<GeneratorYieldSite>,
    next_item: &mut dyn FnMut(GeneratorYieldFromPlanItem<'_>) -> String,
) -> (String, Option<String>) {
    let jump_to_yield_from_label = next_item(GeneratorYieldFromPlanItem::Label);
    let plan = make_generator_yield_from_plan(&mut *next_item, true);
    let result_name = plan.result_name.clone();
    let entry = emit_generator_yield_from_expr_blocks(
        blocks,
        linear,
        value,
        after_label,
        closure_state,
        resume_order,
        yield_sites,
        plan,
        jump_to_yield_from_label,
    );
    (entry, result_name)
}

pub(crate) fn emit_generator_yield_from_assign_blocks(
    blocks: &mut Vec<BlockPyBlock>,
    linear: Vec<Stmt>,
    value: Expr,
    assign_stmt: &ruff_python_ast::StmtAssign,
    cell_slots: &HashSet<String>,
    rest_entry: String,
    closure_state: bool,
    resume_order: &mut Vec<String>,
    yield_sites: &mut Vec<GeneratorYieldSite>,
    plan: GeneratorYieldFromPlan,
    assign_result_label: String,
    jump_to_yield_from_label: String,
) -> String {
    let (yield_from_entry, result_name) = emit_yield_from_blocks(
        value,
        assign_result_label.clone(),
        blocks,
        closure_state,
        resume_order,
        yield_sites,
        plan,
    );
    let result_name = result_name.expect("yield-from assignment lowering requires yielded result");
    let result_expr = py_expr!("{value:id}", value = result_name.as_str());
    let mut final_assign = assign_stmt.clone();
    final_assign.value = Box::new(result_expr);
    let mut assign_body = vec![Stmt::Assign(final_assign.clone())];
    for target in &final_assign.targets {
        assign_body.extend(sync_target_cells_stmts(target, cell_slots));
    }
    blocks.push(compat_jump_block_from_blockpy(
        assign_result_label,
        assign_body,
        rest_entry,
    ));
    blocks.push(compat_jump_block_from_blockpy(
        jump_to_yield_from_label.clone(),
        linear,
        yield_from_entry,
    ));
    jump_to_yield_from_label
}

pub(crate) fn emit_generator_yield_from_return_blocks(
    blocks: &mut Vec<BlockPyBlock>,
    linear: Vec<Stmt>,
    value: Expr,
    closure_state: bool,
    resume_order: &mut Vec<String>,
    yield_sites: &mut Vec<GeneratorYieldSite>,
    plan: GeneratorYieldFromPlan,
    return_label: String,
    jump_to_yield_from_label: String,
) -> String {
    let (yield_from_entry, result_name) = emit_yield_from_blocks(
        value,
        return_label.clone(),
        blocks,
        closure_state,
        resume_order,
        yield_sites,
        plan,
    );
    let result_name = result_name.expect("yield-from return lowering requires yielded result");
    let result_expr = py_expr!("{value:id}", value = result_name.as_str());
    blocks.push(compat_return_block_from_expr(
        return_label,
        Vec::new(),
        Some(result_expr),
    ));
    blocks.push(compat_jump_block_from_blockpy(
        jump_to_yield_from_label.clone(),
        linear,
        yield_from_entry,
    ));
    jump_to_yield_from_label
}

pub(crate) struct GeneratorLoweringCtx<'a> {
    pub blocks: &'a mut Vec<BlockPyBlock>,
    pub closure_state: bool,
    pub resume_order: &'a mut Vec<String>,
    pub yield_sites: &'a mut Vec<GeneratorYieldSite>,
    pub next_item: &'a mut dyn FnMut(GeneratorYieldFromPlanItem<'_>) -> String,
}

#[cfg(test)]
mod tests {
    use super::{build_initial_generator_metadata, GeneratorYieldSite};

    #[test]
    fn initial_generator_metadata_includes_entry_label_in_resume_order() {
        let info = build_initial_generator_metadata(
            "entry",
            true,
            &["resume".to_string()],
            &[GeneratorYieldSite {
                yield_label: "yield_1".to_string(),
                resume_label: "resume".to_string(),
            }],
        );

        assert_eq!(
            info.resume_order
                .iter()
                .map(|label| label.as_str())
                .collect::<Vec<_>>(),
            vec!["entry", "resume"]
        );
        assert_eq!(
            info.yield_sites
                .iter()
                .map(|site| (site.yield_label.as_str(), site.resume_label.as_str()))
                .collect::<Vec<_>>(),
            vec![("yield_1", "resume")]
        );
        assert_eq!(
            info.dispatch_entry_label
                .as_ref()
                .map(|label| label.as_str()),
            Some("entry")
        );
    }
}
