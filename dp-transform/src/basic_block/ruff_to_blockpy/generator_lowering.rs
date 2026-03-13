use super::{
    compat_block_from_blockpy as compat_block_with_term, compat_if_jump_block,
    compat_jump_block_from_blockpy, compat_raise_block_from_blockpy_raise,
    compat_return_block_from_expr, lower_stmts_to_blockpy_stmts, TryRegionPlan,
};
use crate::basic_block::ast_to_ast::scope::cell_name;
use crate::basic_block::bb_ir::{BbClosureInit, BbClosureLayout, BbClosureSlot, FunctionId};
use crate::basic_block::block_py::state::{sync_generator_state_order, sync_target_cells_stmts};
use crate::basic_block::block_py::{
    BlockPyAssign, BlockPyBlock, BlockPyBranchTable, BlockPyCfgBlockBuilder, BlockPyExpr,
    BlockPyIfTerm, BlockPyLabel, BlockPyRaise, BlockPyStmt, BlockPyTerm, BlockPyTryJump,
};
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

fn compat_block_from_blockpy(label: String, body: Vec<Stmt>, term: BlockPyTerm) -> BlockPyBlock {
    compat_block_with_term(label, body, term)
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

#[derive(Debug, Clone)]
pub(crate) struct GeneratorMetadata {
    pub dispatch_entry_label: Option<String>,
    pub resume_order: Vec<String>,
    pub yield_sites: Vec<GeneratorYieldSite>,
    pub done_block_label: Option<String>,
    pub invalid_block_label: Option<String>,
    pub uncaught_block_label: Option<String>,
    pub uncaught_set_done_label: Option<String>,
    pub uncaught_raise_label: Option<String>,
    pub uncaught_exc_name: Option<String>,
    pub dispatch_only_labels: Vec<String>,
    pub throw_passthrough_labels: Vec<String>,
}

pub(crate) struct ClosureBackedGeneratorExportPlan {
    pub factory_label: String,
    pub factory_entry_liveins: Vec<String>,
    pub resume_function_id: FunctionId,
    pub resume_bind_name: String,
    pub resume_display_name: String,
    pub resume_qualname: String,
    pub resume_entry_liveins: Vec<String>,
    pub factory_block: BlockPyBlock,
    pub resume_param_specs: Expr,
}

pub(crate) fn build_async_for_continue_entry(
    blocks: &mut Vec<BlockPyBlock>,
    fn_name: &str,
    iter_expr: Expr,
    tmp_name: &str,
    loop_check_label: &str,
    closure_state: bool,
    try_regions: &mut Vec<TryRegionPlan>,
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
        try_regions,
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
        BlockPyTerm::Jump(BlockPyLabel::from(loop_check_label.to_string())),
    ));
    fetch_entry_label
}

fn closure_backed_generator_init_expr(slot: &BbClosureSlot) -> Expr {
    match slot.init {
        BbClosureInit::InheritedCapture => {
            panic!("inherited captures do not allocate new cells in outer factories")
        }
        BbClosureInit::Parameter => {
            py_expr!("{name:id}", name = slot.logical_name.as_str())
        }
        BbClosureInit::DeletedSentinel => py_expr!("__dp_DELETED"),
        BbClosureInit::RuntimePcUnstarted => py_expr!("1"),
        BbClosureInit::RuntimeNone => py_expr!("None"),
        BbClosureInit::Deferred => py_expr!("None"),
    }
}

fn is_generator_dispatch_param(name: &str) -> bool {
    matches!(
        name,
        "_dp_self" | "_dp_send_value" | "_dp_resume_exc" | "_dp_transport_sent"
    )
}

fn generator_storage_name(name: &str) -> String {
    if name == "_dp_classcell" || name.starts_with("_dp_cell_") {
        return name.to_string();
    }
    cell_name(name)
}

fn logical_name_for_generator_state(name: &str) -> String {
    name.strip_prefix("_dp_cell_").unwrap_or(name).to_string()
}

fn runtime_init(name: &str) -> Option<BbClosureInit> {
    match name {
        "_dp_pc" => Some(BbClosureInit::RuntimePcUnstarted),
        "_dp_yieldfrom" => Some(BbClosureInit::RuntimeNone),
        _ => None,
    }
}

pub(crate) fn build_blockpy_closure_layout(
    param_names: &[String],
    state_vars: &[String],
    capture_names: &[String],
    injected_exception_names: &HashSet<String>,
) -> BbClosureLayout {
    let ordered_state = sync_generator_state_order(state_vars, injected_exception_names);
    let capture_names = capture_names.iter().cloned().collect::<HashSet<_>>();
    let mut seen_storage_names = HashSet::new();

    let mut freevars = Vec::new();
    let mut cellvars = Vec::new();
    let mut runtime_cells = Vec::new();

    for name in ordered_state {
        if is_generator_dispatch_param(name.as_str()) {
            continue;
        }
        let logical_name = logical_name_for_generator_state(name.as_str());
        let storage_name = generator_storage_name(name.as_str());
        if !seen_storage_names.insert(storage_name.clone()) {
            continue;
        }
        if let Some(init) = runtime_init(logical_name.as_str()) {
            runtime_cells.push(BbClosureSlot {
                logical_name,
                storage_name,
                init,
            });
            continue;
        }
        if name == "_dp_classcell"
            || capture_names.contains(name.as_str())
            || capture_names.contains(logical_name.as_str())
        {
            freevars.push(BbClosureSlot {
                logical_name,
                storage_name,
                init: BbClosureInit::InheritedCapture,
            });
            continue;
        }
        let init = if injected_exception_names.contains(logical_name.as_str()) {
            BbClosureInit::DeletedSentinel
        } else if param_names.iter().any(|param| param == &logical_name) {
            BbClosureInit::Parameter
        } else {
            BbClosureInit::Deferred
        };
        cellvars.push(BbClosureSlot {
            logical_name,
            storage_name,
            init,
        });
    }

    BbClosureLayout {
        freevars,
        cellvars,
        runtime_cells,
    }
}

pub(crate) fn closure_backed_generator_resume_state_order(
    layout: &BbClosureLayout,
    is_async_generator: bool,
) -> Vec<String> {
    let mut state_order = vec![
        "_dp_self".to_string(),
        "_dp_send_value".to_string(),
        "_dp_resume_exc".to_string(),
    ];
    if is_async_generator {
        state_order.push("_dp_transport_sent".to_string());
    }
    for slot in layout
        .freevars
        .iter()
        .chain(layout.cellvars.iter())
        .chain(layout.runtime_cells.iter())
    {
        if !state_order.iter().any(|name| name == &slot.storage_name) {
            state_order.push(slot.storage_name.clone());
        }
    }
    state_order
}

fn closure_backed_generator_factory_entry_liveins(
    param_names: &[String],
    layout: &BbClosureLayout,
) -> Vec<String> {
    let mut params = param_names.to_vec();
    for slot in &layout.freevars {
        if !params.iter().any(|name| name == &slot.storage_name) {
            params.push(slot.storage_name.clone());
        }
    }
    params
}

fn closure_backed_generator_resume_param_specs_expr(is_async_generator: bool) -> Expr {
    let mut params = vec![
        blockpy_make_dp_tuple(vec![
            py_expr!("{name:literal}", name = "/_dp_self"),
            py_expr!("None"),
            py_expr!("__dp_NO_DEFAULT"),
        ]),
        blockpy_make_dp_tuple(vec![
            py_expr!("{name:literal}", name = "/_dp_send_value"),
            py_expr!("None"),
            py_expr!("__dp_NO_DEFAULT"),
        ]),
        blockpy_make_dp_tuple(vec![
            py_expr!("{name:literal}", name = "/_dp_resume_exc"),
            py_expr!("None"),
            py_expr!("__dp_NO_DEFAULT"),
        ]),
    ];
    if is_async_generator {
        params.push(blockpy_make_dp_tuple(vec![
            py_expr!("{name:literal}", name = "/_dp_transport_sent"),
            py_expr!("None"),
            py_expr!("__dp_NO_DEFAULT"),
        ]));
    }
    blockpy_make_dp_tuple(params)
}

pub(crate) fn build_closure_backed_generator_export_plan(
    factory_label: &str,
    resume_label: &str,
    resume_function_id: FunctionId,
    bind_name: &str,
    function_name: &str,
    qualname: &str,
    param_names: &[String],
    layout: &BbClosureLayout,
    is_coroutine: bool,
    is_async_generator: bool,
    _target_labels: &[String],
    _resume_pcs: &[(String, usize)],
) -> ClosureBackedGeneratorExportPlan {
    let factory_entry_liveins = closure_backed_generator_factory_entry_liveins(param_names, layout);
    let resume_entry_liveins =
        closure_backed_generator_resume_state_order(layout, is_async_generator);
    let factory_block = build_closure_backed_generator_factory_block(
        factory_label,
        resume_label,
        resume_function_id,
        &resume_entry_liveins,
        function_name,
        qualname,
        layout,
        is_coroutine,
        is_async_generator,
    );
    let resume_param_specs = closure_backed_generator_resume_param_specs_expr(is_async_generator);
    ClosureBackedGeneratorExportPlan {
        factory_label: factory_label.to_string(),
        factory_entry_liveins,
        resume_function_id,
        resume_bind_name: format!("{bind_name}_resume"),
        resume_display_name: "_dp_resume".to_string(),
        resume_qualname: qualname.to_string(),
        resume_entry_liveins,
        factory_block,
        resume_param_specs,
    }
}

pub(crate) fn build_closure_backed_generator_factory_block(
    factory_label: &str,
    resume_label: &str,
    resume_function_id: FunctionId,
    resume_state_order: &[String],
    function_name: &str,
    qualname: &str,
    layout: &BbClosureLayout,
    is_coroutine: bool,
    is_async_generator: bool,
) -> BlockPyBlock {
    let hidden_name = "_dp_resume".to_string();
    let hidden_qualname = qualname.to_string();
    let mut body = Vec::new();

    for slot in layout.cellvars.iter().chain(layout.runtime_cells.iter()) {
        let stmt = py_stmt!(
            "{cell:id} = __dp_make_cell({init:expr})",
            cell = slot.storage_name.as_str(),
            init = closure_backed_generator_init_expr(slot),
        );
        let lowered = lower_stmts_to_blockpy_stmts(&[stmt])
            .unwrap_or_else(|err| panic!("failed to lower generator factory cell init: {err}"));
        assert!(lowered.term.is_none());
        body.extend(lowered.body);
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
        "__dp_def_hidden_resume_fn({resume:literal}, {function_id:literal}, {name:literal}, {qualname:literal}, {state_order:expr}, {closure_names:expr}, {closure_values:expr}, __dp_globals(), __name__, async_gen={async_gen:expr})",
        resume = resume_label,
        function_id = resume_function_id.0,
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

    let mut block = BlockPyCfgBlockBuilder::<BlockPyStmt, BlockPyTerm>::new(factory_label.into());
    block.extend(body);
    block.set_term(BlockPyTerm::Return(Some(return_value.into())));
    block.finish(None)
}

pub(crate) fn build_initial_generator_metadata(
    dispatch_entry_label: &str,
    resume_order: &[String],
    yield_sites: &[GeneratorYieldSite],
) -> GeneratorMetadata {
    let mut resume_order = resume_order.iter().cloned().collect::<Vec<_>>();
    if !resume_order
        .iter()
        .any(|label| label == dispatch_entry_label)
    {
        resume_order.insert(0, dispatch_entry_label.to_string());
    }
    GeneratorMetadata {
        dispatch_entry_label: Some(dispatch_entry_label.to_string()),
        resume_order,
        yield_sites: yield_sites.to_vec(),
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
    resume_order: &[String],
    yield_sites: &[GeneratorYieldSite],
) -> GeneratorMetadata {
    let dispatch_info = synthesize_generator_dispatch(
        blocks,
        entry_label,
        label_prefix,
        is_async_generator_runtime,
        is_closure_backed_generator_runtime,
        uncaught_exc_name,
        resume_order,
    );
    GeneratorMetadata {
        dispatch_entry_label: dispatch_info.generator_resume_entry_label.clone(),
        resume_order: dispatch_info.generator_resume_order.clone(),
        yield_sites: yield_sites.to_vec(),
        done_block_label: dispatch_info.done_block_label.clone(),
        invalid_block_label: dispatch_info.invalid_block_label.clone(),
        uncaught_block_label: dispatch_info.generator_uncaught_label.clone(),
        uncaught_set_done_label: dispatch_info.generator_uncaught_set_done_label.clone(),
        uncaught_raise_label: dispatch_info.generator_uncaught_raise_label.clone(),
        uncaught_exc_name: dispatch_info.generator_uncaught_exc_name.clone(),
        dispatch_only_labels: dispatch_info
            .generator_dispatch_only_labels
            .iter()
            .cloned()
            .collect(),
        throw_passthrough_labels: dispatch_info
            .generator_throw_passthrough_labels
            .iter()
            .cloned()
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
    ambient_exc_param: Option<&str>,
    closure_state: bool,
    try_regions: &mut Vec<TryRegionPlan>,
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
        try_regions,
        resume_order,
        yield_sites,
        next_item: &mut next_item,
    };
    let generated_start = ctx.blocks.len();
    let entry = match stmt {
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
                ctx.try_regions,
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
                ctx.try_regions,
                ctx.resume_order,
                ctx.yield_sites,
                plan,
                assign_result_label,
                label,
            );
            Some(lowered)
        }
        _ => None,
    };
    if let Some(exc_param) = ambient_exc_param {
        for block in &mut ctx.blocks[generated_start..] {
            if block.meta.exc_param.is_none() {
                block.meta.exc_param = Some(exc_param.to_string());
            }
        }
    }
    entry
}

pub(crate) fn lower_generator_blockpy_term_in_sequence(
    term: &BlockPyTerm,
    linear: Vec<Stmt>,
    blocks: &mut Vec<BlockPyBlock>,
    ambient_exc_param: Option<&str>,
    closure_state: bool,
    try_regions: &mut Vec<TryRegionPlan>,
    resume_order: &mut Vec<String>,
    yield_sites: &mut Vec<GeneratorYieldSite>,
    next_block_id: &mut usize,
    fn_name: &str,
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
        try_regions,
        resume_order,
        yield_sites,
        next_item: &mut next_item,
    };
    let generated_start = ctx.blocks.len();
    let entry = match term {
        BlockPyTerm::Return(Some(BlockPyExpr::Yield(yield_expr))) => {
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
        BlockPyTerm::Return(Some(BlockPyExpr::YieldFrom(yield_from_expr))) => {
            let return_label = (ctx.next_item)(GeneratorYieldFromPlanItem::Label);
            let label = (ctx.next_item)(GeneratorYieldFromPlanItem::Label);
            let plan = make_generator_yield_from_plan(&mut *ctx.next_item, true);
            let lowered = emit_generator_yield_from_return_blocks(
                ctx.blocks,
                linear,
                *yield_from_expr.value.clone(),
                ctx.closure_state,
                ctx.try_regions,
                ctx.resume_order,
                ctx.yield_sites,
                plan,
                return_label,
                label,
            );
            Some(lowered)
        }
        _ => None,
    };
    if let Some(exc_param) = ambient_exc_param {
        for block in &mut ctx.blocks[generated_start..] {
            if block.meta.exc_param.is_none() {
                block.meta.exc_param = Some(exc_param.to_string());
            }
        }
    }
    entry
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

    let resume_dispatch_label = format!("{label_prefix}_dispatch");
    let resume_send_table_label = format!("{label_prefix}_dispatch_send_table");
    let resume_throw_table_label = format!("{label_prefix}_dispatch_throw_table");
    let resume_send_label = format!("{label_prefix}_dispatch_send");
    let resume_send_precheck_value_label = format!("{label_prefix}_dispatch_send_precheck_value");
    let resume_send_precheck_transport_label =
        format!("{label_prefix}_dispatch_send_precheck_transport");
    let resume_send_transport_error_label = format!("{label_prefix}_dispatch_send_transport_error");
    info.generator_dispatch_only_labels.extend([
        resume_dispatch_label.clone(),
        resume_send_table_label.clone(),
        resume_throw_table_label.clone(),
    ]);
    if is_async_generator_runtime {
        info.generator_dispatch_only_labels.extend([
            resume_send_label.clone(),
            resume_send_precheck_value_label.clone(),
            resume_send_precheck_transport_label.clone(),
            resume_send_transport_error_label.clone(),
        ]);
    }

    let mut send_table_targets = vec![BlockPyLabel::from(done_label.clone())];
    let mut throw_table_targets = vec![BlockPyLabel::from(resume_throw_done_label.clone())];
    for (resume_index, resume_target) in resume_order.iter().enumerate() {
        let pc = resume_index + 1;
        let throw_target = if pc == 1 {
            resume_throw_unstarted_label.clone()
        } else {
            resume_target.clone()
        };
        send_table_targets.push(BlockPyLabel::from(resume_target.clone()));
        throw_table_targets.push(BlockPyLabel::from(throw_target));
    }

    blocks.push(compat_block_from_blockpy(
        resume_send_table_label.clone(),
        Vec::new(),
        BlockPyTerm::BranchTable(BlockPyBranchTable {
            index: generator_pc_expr.clone().into(),
            targets: send_table_targets,
            default_label: BlockPyLabel::from(invalid_label.clone()),
        }),
    ));
    blocks.push(compat_block_from_blockpy(
        resume_throw_table_label.clone(),
        Vec::new(),
        BlockPyTerm::BranchTable(BlockPyBranchTable {
            index: generator_pc_expr.clone().into(),
            targets: throw_table_targets,
            default_label: BlockPyLabel::from(invalid_label),
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
            BlockPyTerm::Raise(blockpy_raise_from_stmt(transport_error_raise_stmt)),
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
            resume_send_label.clone(),
            Vec::new(),
            py_expr!("__dp_eq({pc:expr}, 1)", pc = generator_pc_expr.clone()),
            resume_send_precheck_value_label,
            resume_send_table_label.clone(),
        ));
    }
    let send_dispatch_entry_label = if is_async_generator_runtime {
        resume_send_label
    } else {
        resume_send_table_label.clone()
    };
    blocks.push(compat_if_jump_block(
        resume_dispatch_label.clone(),
        Vec::new(),
        py_expr!("__dp_is_(_dp_resume_exc, None)"),
        send_dispatch_entry_label,
        resume_throw_table_label,
    ));
    *entry_label = resume_dispatch_label;

    info
}

fn lower_generated_stmts_to_blockpy(stmts: Vec<Stmt>) -> Vec<BlockPyStmt> {
    let lowered = lower_stmts_to_blockpy_stmts(&stmts)
        .unwrap_or_else(|err| panic!("failed to convert generated stmt to BlockPy: {err}"));
    assert!(lowered.term.is_none());
    lowered.body
}

pub(crate) fn lower_generator_yield_terms_to_explicit_return_blockpy(
    blocks: &mut [BlockPyBlock],
    block_params: &HashMap<String, Vec<String>>,
    resume_pcs: &[(String, usize)],
    yield_sites: &[GeneratorYieldSite],
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
        match &mut block.term {
            BlockPyTerm::Return(yielded_value)
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
                block.body.extend(injected);
            }
            BlockPyTerm::Return(_) => {}
            _ => {}
        }
    }
}

pub(crate) fn split_generator_return_terms_to_escape_blocks(
    blocks: &mut Vec<BlockPyBlock>,
    yield_sites: &[GeneratorYieldSite],
    is_async: bool,
    closure_state: bool,
) -> HashSet<String> {
    let yield_labels = yield_sites
        .iter()
        .map(|site| site.yield_label.as_str().to_string())
        .collect::<HashSet<_>>();
    let mut known_labels = blocks
        .iter()
        .map(|block| block.label.as_str().to_string())
        .collect::<HashSet<_>>();
    let mut escape_labels = HashSet::new();
    let mut extra_blocks = Vec::new();

    for block in blocks.iter_mut() {
        let BlockPyTerm::Return(value) = &block.term else {
            continue;
        };
        if yield_labels.contains(block.label.as_str()) {
            continue;
        }

        let base_label = format!("{}_return_done", block.label.as_str());
        let mut return_label = base_label.clone();
        let mut suffix = 0usize;
        while known_labels.contains(return_label.as_str()) {
            suffix += 1;
            return_label = format!("{base_label}_{suffix}");
        }
        known_labels.insert(return_label.clone());
        escape_labels.insert(return_label.clone());

        let mut injected = if closure_state {
            lower_generated_stmts_to_blockpy(vec![py_stmt!(
                "__dp_store_cell(_dp_cell__dp_pc, __dp_GEN_PC_DONE)"
            )])
        } else {
            lower_generated_stmts_to_blockpy(vec![py_stmt!(
                "__dp_setattr(_dp_self, \"_pc\", __dp_GEN_PC_DONE)"
            )])
        };
        block.body.append(&mut injected);

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
        block.term = BlockPyTerm::Jump(BlockPyLabel::from(return_label.clone()));
        extra_blocks.push(compat_raise_block_from_blockpy_raise(
            return_label,
            Vec::new(),
            blockpy_raise_from_stmt(raise_stmt),
        ));
    }

    blocks.extend(extra_blocks);
    escape_labels
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
        BlockPyTerm::Raise(BlockPyRaise {
            exc: Some(py_expr!("{name:id}", name = "_dp_resume_exc").into()),
        }),
    ));
    blocks.push(compat_block_with_term(
        resume_dispatch_label.clone(),
        Vec::new(),
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test: py_expr!("__dp_is_not(_dp_resume_exc, None)").into(),
            then_label: BlockPyLabel::from(resume_raise_label),
            else_label: BlockPyLabel::from(resume_success_label),
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
    try_regions: &mut Vec<TryRegionPlan>,
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

    try_regions.push(TryRegionPlan {
        body_region_labels: vec![plan.next_body_label.clone()],
        body_exception_target: plan.stop_except_label.clone(),
        cleanup_region_labels: Vec::new(),
        cleanup_exception_target: None,
    });
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
        BlockPyTerm::TryJump(BlockPyTryJump {
            body_label: BlockPyLabel::from(plan.next_body_label.clone()),
            except_label: BlockPyLabel::from(plan.stop_except_label.clone()),
        }),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.next_body_label.clone(),
        vec![py_stmt!(
            "{yielded:id} = next({iter_expr:expr})",
            yielded = plan.yielded_name.as_str(),
            iter_expr = yieldfrom_load_expr.clone(),
        )],
        BlockPyTerm::Jump(BlockPyLabel::from(plan.yield_label.clone())),
    ));
    let mut stop_except_block = compat_if_jump_block(
        plan.stop_except_label.clone(),
        vec![py_stmt!(
            "{stop:id} = __dp_current_exception()",
            stop = plan.stop_name.as_str(),
        )],
        py_expr!(
            "__dp_exception_matches({stop:id}, StopIteration)",
            stop = plan.stop_name.as_str(),
        ),
        plan.stop_done_label.clone(),
        plan.raise_stop_label.clone(),
    );
    stop_except_block.meta.exc_param = Some(plan.stop_name.clone());
    blocks.push(stop_except_block);
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
        BlockPyTerm::Jump(BlockPyLabel::from(plan.clear_done_label.clone())),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.clear_done_label,
        vec![yieldfrom_clear_stmt.clone()],
        BlockPyTerm::Jump(BlockPyLabel::from(after_label)),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.raise_stop_label.clone(),
        vec![py_stmt!(
            "{raise:id} = {stop:id}",
            raise = plan.raise_name.as_str(),
            stop = plan.stop_name.as_str(),
        )],
        BlockPyTerm::Jump(BlockPyLabel::from(plan.clear_raise_label.clone())),
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
        BlockPyTerm::Return(Some(
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
    blocks.push(compat_block_with_term(
        plan.genexit_close_lookup_label.clone(),
        vec![py_stmt!(
            "{close:id} = getattr({iter_expr:expr}, \"close\", None)",
            close = plan.close_name.as_str(),
            iter_expr = yieldfrom_load_raw_expr.clone(),
        )],
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test: py_expr!(
                "__dp_is_not({close:id}, None)",
                close = plan.close_name.as_str()
            )
            .into(),
            then_label: BlockPyLabel::from(plan.genexit_call_close_label.clone()),
            else_label: BlockPyLabel::from(plan.raise_exc_label.clone()),
        }),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.genexit_call_close_label,
        vec![py_stmt!("{close:id}()", close = plan.close_name.as_str())],
        BlockPyTerm::Jump(BlockPyLabel::from(plan.raise_exc_label.clone())),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.raise_exc_label.clone(),
        vec![py_stmt!(
            "{raise:id} = {exc:id}",
            raise = plan.raise_name.as_str(),
            exc = plan.exc_name.as_str(),
        )],
        BlockPyTerm::Jump(BlockPyLabel::from(plan.clear_raise_label.clone())),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.clear_raise_label,
        vec![yieldfrom_clear_stmt],
        BlockPyTerm::Raise(BlockPyRaise {
            exc: Some(py_expr!("{name:id}", name = plan.raise_name.as_str()).into()),
        }),
    ));
    blocks.push(compat_block_with_term(
        plan.lookup_throw_label.clone(),
        vec![py_stmt!(
            "{throw:id} = getattr({iter_expr:expr}, \"throw\", None)",
            throw = plan.throw_name.as_str(),
            iter_expr = yieldfrom_load_raw_expr,
        )],
        BlockPyTerm::IfTerm(BlockPyIfTerm {
            test: py_expr!(
                "__dp_is_({throw:id}, None)",
                throw = plan.throw_name.as_str()
            )
            .into(),
            then_label: BlockPyLabel::from(plan.raise_exc_label),
            else_label: BlockPyLabel::from(plan.throw_try_label.clone()),
        }),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.throw_try_label,
        Vec::new(),
        BlockPyTerm::TryJump(BlockPyTryJump {
            body_label: BlockPyLabel::from(plan.throw_body_label.clone()),
            except_label: BlockPyLabel::from(plan.stop_except_label.clone()),
        }),
    ));
    try_regions.push(TryRegionPlan {
        body_region_labels: vec![plan.throw_body_label.clone()],
        body_exception_target: plan.stop_except_label.clone(),
        cleanup_region_labels: Vec::new(),
        cleanup_exception_target: None,
    });
    blocks.push(compat_block_from_blockpy(
        plan.throw_body_label,
        vec![py_stmt!(
            "{yielded:id} = {throw:id}({exc:id})",
            yielded = plan.yielded_name.as_str(),
            throw = plan.throw_name.as_str(),
            exc = plan.exc_name.as_str(),
        )],
        BlockPyTerm::Jump(BlockPyLabel::from(yield_label.clone())),
    ));
    blocks.push(compat_block_from_blockpy(
        plan.send_try_label,
        Vec::new(),
        BlockPyTerm::TryJump(BlockPyTryJump {
            body_label: BlockPyLabel::from(plan.send_dispatch_label.clone()),
            except_label: BlockPyLabel::from(plan.stop_except_label.clone()),
        }),
    ));
    try_regions.push(TryRegionPlan {
        body_region_labels: vec![
            plan.send_dispatch_label.clone(),
            plan.next_body_label.clone(),
            plan.send_call_body_label.clone(),
        ],
        body_exception_target: plan.stop_except_label.clone(),
        cleanup_region_labels: Vec::new(),
        cleanup_exception_target: None,
    });
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
        BlockPyTerm::Jump(BlockPyLabel::from(yield_label)),
    ));
    (plan.init_try_label, plan.result_name)
}

pub(crate) fn emit_generator_yield_from_expr_blocks(
    blocks: &mut Vec<BlockPyBlock>,
    linear: Vec<Stmt>,
    value: Expr,
    after_label: String,
    closure_state: bool,
    try_regions: &mut Vec<TryRegionPlan>,
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
        try_regions,
        resume_order,
        yield_sites,
        plan,
    );
    blocks.push(compat_block_from_blockpy(
        jump_to_yield_from_label.clone(),
        linear,
        BlockPyTerm::Jump(BlockPyLabel::from(yield_from_entry)),
    ));
    jump_to_yield_from_label
}

pub(crate) fn lower_generator_yield_from_value(
    blocks: &mut Vec<BlockPyBlock>,
    linear: Vec<Stmt>,
    value: Expr,
    after_label: String,
    closure_state: bool,
    try_regions: &mut Vec<TryRegionPlan>,
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
        try_regions,
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
    try_regions: &mut Vec<TryRegionPlan>,
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
        try_regions,
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
    try_regions: &mut Vec<TryRegionPlan>,
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
        try_regions,
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
    pub try_regions: &'a mut Vec<TryRegionPlan>,
    pub resume_order: &'a mut Vec<String>,
    pub yield_sites: &'a mut Vec<GeneratorYieldSite>,
    pub next_item: &'a mut dyn FnMut(GeneratorYieldFromPlanItem<'_>) -> String,
}

#[cfg(test)]
mod tests {
    use super::{
        build_closure_backed_generator_export_plan, build_initial_generator_metadata,
        GeneratorYieldSite,
    };
    use crate::basic_block::bb_ir::{BbClosureInit, BbClosureLayout, BbClosureSlot};

    #[test]
    fn initial_generator_metadata_includes_entry_label_in_resume_order() {
        let info = build_initial_generator_metadata(
            "entry",
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
        assert_eq!(info.dispatch_entry_label.as_deref(), Some("entry"));
    }

    #[test]
    fn closure_backed_export_plan_includes_capture_params_and_resume_state() {
        let layout = BbClosureLayout {
            freevars: vec![BbClosureSlot {
                logical_name: "factor".to_string(),
                storage_name: "_dp_cell_factor".to_string(),
                init: BbClosureInit::InheritedCapture,
            }],
            cellvars: vec![BbClosureSlot {
                logical_name: "total".to_string(),
                storage_name: "_dp_cell_total".to_string(),
                init: BbClosureInit::Deferred,
            }],
            runtime_cells: vec![BbClosureSlot {
                logical_name: "_dp_pc".to_string(),
                storage_name: "_dp_cell__dp_pc".to_string(),
                init: BbClosureInit::RuntimePcUnstarted,
            }],
        };

        let plan = build_closure_backed_generator_export_plan(
            "_dp_bb_agen_factory",
            "_dp_bb_agen_start",
            crate::basic_block::bb_ir::FunctionId(7),
            "agen",
            "agen",
            "outer.<locals>.agen",
            &["scale".to_string()],
            &layout,
            false,
            true,
            &[
                "_dp_bb_agen_start".to_string(),
                "_dp_bb_agen_resume".to_string(),
            ],
            &[
                ("_dp_bb_agen_start".to_string(), 0),
                ("_dp_bb_agen_resume".to_string(), 1),
            ],
        );

        assert_eq!(plan.factory_label, "_dp_bb_agen_factory");
        assert_eq!(plan.factory_entry_liveins, vec!["scale", "_dp_cell_factor"]);
        assert_eq!(plan.resume_bind_name, "agen_resume");
        assert_eq!(plan.resume_display_name, "_dp_resume");
        assert_eq!(plan.resume_qualname, "outer.<locals>.agen");
        assert_eq!(
            plan.resume_entry_liveins,
            vec![
                "_dp_self",
                "_dp_send_value",
                "_dp_resume_exc",
                "_dp_transport_sent",
                "_dp_cell_factor",
                "_dp_cell_total",
                "_dp_cell__dp_pc",
            ]
        );
        assert_eq!(plan.factory_block.label.as_str(), "_dp_bb_agen_factory");
    }
}
