use crate::block_py::cfg::linearize_structured_ifs;
use crate::block_py::dataflow::analyze_blockpy_use_def;
use crate::block_py::param_specs::{Param, ParamKind, ParamSpec};
use crate::block_py::state::collect_state_vars;
use crate::block_py::{
    core_positional_call_expr_with_meta, is_resume_abi_param_name, resume_abi_params, BlockParam,
    BlockParamRole, BlockPyAssign, BlockPyBlock, BlockPyBranchTable, BlockPyCfgBlockBuilder,
    BlockPyCfgFragment, BlockPyFunction, BlockPyFunctionKind, BlockPyIf, BlockPyIfTerm,
    BlockPyLabel, BlockPyRaise, BlockPyStmt, BlockPyTerm, CfgBlock, ClosureInit, ClosureLayout,
    ClosureSlot, CoreBlockPyExpr, CoreBlockPyExprWithAwaitAndYield, CoreBlockPyExprWithYield,
    FunctionId, FunctionName,
};
use crate::passes::ast_to_ast::expr_utils::make_dp_tuple;
use crate::passes::ast_to_ast::scope::cell_name;
use crate::passes::core_eval_order::make_eval_order_explicit_in_core_block_without_await;
use crate::passes::ruff_to_blockpy::{
    attach_exception_edges_to_blocks, lowered_exception_edges, recompute_lowered_block_params,
    should_include_closure_storage_aliases,
};
use crate::passes::{CoreBlockPyPass, CoreBlockPyPassWithYield};
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr, ExprName};
use std::collections::HashSet;
use std::collections::{HashMap, VecDeque};

fn generator_storage_name(name: &str) -> String {
    if name == "_dp_classcell" || name.starts_with("_dp_cell_") {
        return name.to_string();
    }
    cell_name(name)
}

fn logical_name_for_generator_state(name: &str) -> String {
    name.strip_prefix("_dp_cell_").unwrap_or(name).to_string()
}

fn runtime_init(name: &str) -> Option<ClosureInit> {
    match name {
        "_dp_pc" => Some(ClosureInit::RuntimePcUnstarted),
        "_dp_yieldfrom" => Some(ClosureInit::RuntimeNone),
        _ => None,
    }
}

pub(crate) fn build_blockpy_closure_layout(
    param_names: &[String],
    state_vars: &[String],
    capture_names: &[String],
    injected_exception_names: &HashSet<String>,
) -> ClosureLayout {
    let ordered_state = state_vars
        .iter()
        .filter(|name| !is_resume_abi_param_name(name.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let capture_names = capture_names.iter().cloned().collect::<HashSet<_>>();
    let mut seen_storage_names = HashSet::new();

    let mut freevars = Vec::new();
    let mut cellvars = Vec::new();
    let mut runtime_cells = Vec::new();

    for name in ordered_state {
        let logical_name = logical_name_for_generator_state(name.as_str());
        let storage_name = generator_storage_name(name.as_str());
        if !seen_storage_names.insert(storage_name.clone()) {
            continue;
        }
        if let Some(init) = runtime_init(logical_name.as_str()) {
            runtime_cells.push(ClosureSlot {
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
            freevars.push(ClosureSlot {
                logical_name,
                storage_name,
                init: ClosureInit::InheritedCapture,
            });
            continue;
        }
        let init = if injected_exception_names.contains(logical_name.as_str()) {
            ClosureInit::DeletedSentinel
        } else if param_names.iter().any(|param| param == &logical_name) {
            ClosureInit::Parameter
        } else {
            ClosureInit::Deferred
        };
        cellvars.push(ClosureSlot {
            logical_name,
            storage_name,
            init,
        });
    }

    ClosureLayout {
        freevars,
        cellvars,
        runtime_cells,
    }
}

fn expr_name(id: &str) -> ExprName {
    let Expr::Name(expr) = py_expr!("{id:id}", id = id) else {
        unreachable!();
    };
    expr
}

fn core_expr_without_yield(expr: Expr) -> CoreBlockPyExpr {
    let core = CoreBlockPyExprWithAwaitAndYield::from(expr);
    let core_without_await: CoreBlockPyExprWithYield = core
        .try_into()
        .unwrap_or_else(|_| panic!("generator helper expression unexpectedly contained await"));
    core_without_await
        .try_into()
        .unwrap_or_else(|_| panic!("generator helper expression unexpectedly contained yield"))
}

fn core_name(name: &str) -> CoreBlockPyExpr {
    core_expr_without_yield(py_expr!("{name:id}", name = name))
}

fn core_literal_int(value: usize) -> CoreBlockPyExpr {
    core_expr_without_yield(py_expr!("{value:literal}", value = value))
}

fn core_none() -> CoreBlockPyExpr {
    core_expr_without_yield(py_expr!("None"))
}

fn core_call(func_name: &str, args: Vec<CoreBlockPyExpr>) -> CoreBlockPyExpr {
    core_positional_call_expr_with_meta(
        func_name,
        ast::AtomicNodeIndex::default(),
        Default::default(),
        args,
    )
}

fn runtime_init_expr(slot: &ClosureSlot) -> CoreBlockPyExpr {
    match slot.init {
        ClosureInit::InheritedCapture => {
            panic!("inherited captures do not allocate new cells in outer factories")
        }
        ClosureInit::Parameter => core_name(slot.logical_name.as_str()),
        ClosureInit::DeletedSentinel => core_expr_without_yield(py_expr!("__dp_DELETED")),
        ClosureInit::RuntimePcUnstarted => core_literal_int(1),
        ClosureInit::RuntimeNone | ClosureInit::Deferred => core_none(),
    }
}

fn is_generator_like(kind: BlockPyFunctionKind) -> bool {
    matches!(
        kind,
        BlockPyFunctionKind::Generator
            | BlockPyFunctionKind::Coroutine
            | BlockPyFunctionKind::AsyncGenerator
    )
}

fn is_async_generator(kind: BlockPyFunctionKind) -> bool {
    matches!(kind, BlockPyFunctionKind::AsyncGenerator)
}

fn injected_exception_names(
    blocks: &[CfgBlock<
        BlockPyStmt<CoreBlockPyExprWithYield>,
        BlockPyTerm<CoreBlockPyExprWithYield>,
    >],
) -> HashSet<String> {
    let mut names = HashSet::new();
    for block in blocks {
        if let Some(exc_param) = block.exception_param() {
            names.insert(exc_param.to_string());
        }
    }
    names
}

fn build_generator_closure_layout(
    callable: &BlockPyFunction<CoreBlockPyPassWithYield>,
) -> ClosureLayout {
    let entry_liveins = callable.entry_liveins();
    let param_names = callable.params.names();
    let mut local_cell_slots = callable
        .facts
        .cell_slots
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    local_cell_slots.sort();
    let param_name_set = param_names.iter().cloned().collect::<HashSet<_>>();
    let locally_assigned: HashSet<String> = callable
        .blocks
        .iter()
        .flat_map(|block| analyze_blockpy_use_def(block).1.into_iter())
        .collect();
    let mut capture_names = entry_liveins
        .iter()
        .filter(|name| !param_name_set.contains(name.as_str()))
        .filter(|name| {
            *name == "_dp_classcell"
                || (name.starts_with("_dp_cell_")
                    && !callable.facts.cell_slots.contains(name.as_str()))
                || (!name.starts_with("_dp_") && !locally_assigned.contains(name.as_str()))
        })
        .cloned()
        .collect::<Vec<_>>();
    capture_names.sort();
    capture_names.dedup();

    let mut state_vars = collect_state_vars(&param_names, &callable.blocks);
    for block in &callable.blocks {
        if let Some(exc_param) = block.exception_param() {
            if !state_vars.iter().any(|existing| existing == exc_param) {
                state_vars.push(exc_param.to_string());
            }
        }
    }
    for slot in local_cell_slots {
        let logical_name = slot.strip_prefix("_dp_cell_").unwrap_or(&slot).to_string();
        if !state_vars.iter().any(|existing| existing == &logical_name) {
            state_vars.push(logical_name);
        }
    }
    for runtime_name in ["_dp_pc", "_dp_yieldfrom"] {
        if !state_vars.iter().any(|existing| existing == runtime_name) {
            state_vars.push(runtime_name.to_string());
        }
    }

    build_blockpy_closure_layout(
        &param_names,
        &state_vars,
        &capture_names,
        &injected_exception_names(&callable.blocks),
    )
}

fn generator_state_order(layout: &ClosureLayout, kind: BlockPyFunctionKind) -> Vec<String> {
    let mut order = resume_abi_params(kind)
        .iter()
        .map(|param| param.name().to_string())
        .collect::<Vec<_>>();
    order.extend(layout.freevars.iter().map(|slot| slot.storage_name.clone()));
    order.extend(layout.cellvars.iter().map(|slot| slot.logical_name.clone()));
    order.extend(
        layout
            .runtime_cells
            .iter()
            .map(|slot| slot.logical_name.clone()),
    );
    order
}

fn closure_value_name_for_state(layout: &ClosureLayout, state_name: &str) -> String {
    if let Some(slot) = layout
        .freevars
        .iter()
        .chain(layout.cellvars.iter())
        .chain(layout.runtime_cells.iter())
        .find(|slot| slot.logical_name == state_name || slot.storage_name == state_name)
    {
        match slot.init {
            ClosureInit::InheritedCapture => slot.storage_name.clone(),
            _ => slot.storage_name.clone(),
        }
    } else {
        state_name.to_string()
    }
}

fn resume_closure_names(layout: &ClosureLayout, resume_state_order: &[String]) -> Vec<String> {
    let mut names = resume_state_order
        .iter()
        .filter(|name| !is_resume_abi_param_name(name.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let mut seen = names.iter().cloned().collect::<HashSet<_>>();
    for slot in layout.cellvars.iter().chain(layout.runtime_cells.iter()) {
        if slot.storage_name != slot.logical_name && seen.insert(slot.storage_name.clone()) {
            names.push(slot.storage_name.clone());
        }
    }
    names
}

fn generator_cell_storage_by_logical_name(layout: &ClosureLayout) -> HashMap<String, String> {
    layout
        .freevars
        .iter()
        .chain(layout.cellvars.iter())
        .chain(layout.runtime_cells.iter())
        .filter(|slot| slot.storage_name != slot.logical_name)
        .map(|slot| (slot.logical_name.clone(), slot.storage_name.clone()))
        .collect()
}

fn sync_resume_state_fragment(
    fragment: BlockPyCfgFragment<BlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>>,
    storage_by_logical_name: &HashMap<String, String>,
) -> BlockPyCfgFragment<BlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>> {
    BlockPyCfgFragment {
        body: sync_resume_state_body(fragment.body, storage_by_logical_name),
        term: fragment.term,
    }
}

fn sync_resume_state_stmt(
    stmt: BlockPyStmt<CoreBlockPyExpr>,
    storage_by_logical_name: &HashMap<String, String>,
) -> Vec<BlockPyStmt<CoreBlockPyExpr>> {
    match stmt {
        BlockPyStmt::Assign(assign) => {
            let target_name = assign.target.id.to_string();
            let mut synced = vec![BlockPyStmt::Assign(assign)];
            if let Some(storage_name) = storage_by_logical_name.get(target_name.as_str()) {
                synced.push(BlockPyStmt::Expr(core_call(
                    "__dp_store_cell",
                    vec![
                        core_name(storage_name.as_str()),
                        core_name(target_name.as_str()),
                    ],
                )));
            }
            synced
        }
        BlockPyStmt::If(if_stmt) => vec![BlockPyStmt::If(BlockPyIf {
            test: if_stmt.test,
            body: sync_resume_state_fragment(if_stmt.body, storage_by_logical_name),
            orelse: sync_resume_state_fragment(if_stmt.orelse, storage_by_logical_name),
        })],
        BlockPyStmt::Delete(delete) => vec![BlockPyStmt::Delete(delete)],
        BlockPyStmt::Expr(expr) => vec![BlockPyStmt::Expr(expr)],
    }
}

fn sync_resume_state_body(
    body: Vec<BlockPyStmt<CoreBlockPyExpr>>,
    storage_by_logical_name: &HashMap<String, String>,
) -> Vec<BlockPyStmt<CoreBlockPyExpr>> {
    let mut synced = Vec::new();
    for stmt in body {
        synced.extend(sync_resume_state_stmt(stmt, storage_by_logical_name));
    }
    synced
}

fn sync_resume_state_blocks(
    blocks: Vec<CfgBlock<BlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>>>,
    layout: &ClosureLayout,
) -> Vec<CfgBlock<BlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>>> {
    let storage_by_logical_name = generator_cell_storage_by_logical_name(layout);
    if storage_by_logical_name.is_empty() {
        return blocks;
    }
    blocks
        .into_iter()
        .map(|block| CfgBlock {
            label: block.label,
            body: sync_resume_state_body(block.body, &storage_by_logical_name),
            term: block.term,
            params: block.params,
            exc_edge: block.exc_edge,
        })
        .collect()
}

fn build_factory_block(
    visible_function_id: FunctionId,
    resume_function_id: FunctionId,
    resume_state_order: &[String],
    layout: &ClosureLayout,
    kind: BlockPyFunctionKind,
) -> BlockPyBlock<CoreBlockPyExpr> {
    let mut block = BlockPyCfgBlockBuilder::new(BlockPyLabel::from("_dp_factory_entry"));

    for slot in layout.cellvars.iter().chain(layout.runtime_cells.iter()) {
        block.push_stmt(BlockPyStmt::Assign(BlockPyAssign {
            target: expr_name(slot.storage_name.as_str()),
            value: core_call("__dp_make_cell", vec![runtime_init_expr(slot)]),
        }));
    }

    let closure_names = resume_closure_names(layout, resume_state_order);
    let closure_values = closure_names
        .iter()
        .map(|state_name| {
            Expr::from(core_name(
                closure_value_name_for_state(layout, state_name.as_str()).as_str(),
            ))
        })
        .collect::<Vec<_>>();

    let resume_entry = core_expr_without_yield(py_expr!(
        "__dp_def_hidden_resume_fn({function_id:literal}, {closure_names:expr}, {closure_values:expr}, __dp_globals(), async_gen={async_gen:expr})",
        function_id = resume_function_id.0,
        closure_names = make_dp_tuple(
            closure_names
                .iter()
                .map(|value| py_expr!("{value:literal}", value = value.as_str()))
                .collect(),
        ),
        closure_values = make_dp_tuple(closure_values),
        async_gen = if is_async_generator(kind) {
            py_expr!("True")
        } else {
            py_expr!("False")
        },
    ));

    let factory_value = match kind {
        BlockPyFunctionKind::Generator => core_call(
            "__dp_make_closure_generator",
            vec![
                core_expr_without_yield(py_expr!("{value:literal}", value = visible_function_id.0)),
                resume_entry,
                core_expr_without_yield(py_expr!("__dp_globals()")),
            ],
        ),
        BlockPyFunctionKind::Coroutine => core_call(
            "__dp_make_coroutine_from_generator",
            vec![core_call(
                "__dp_make_closure_generator",
                vec![
                    core_expr_without_yield(py_expr!(
                        "{value:literal}",
                        value = visible_function_id.0
                    )),
                    resume_entry,
                    core_expr_without_yield(py_expr!("__dp_globals()")),
                ],
            )],
        ),
        BlockPyFunctionKind::AsyncGenerator => core_call(
            "__dp_make_closure_async_generator",
            vec![
                core_expr_without_yield(py_expr!("{value:literal}", value = visible_function_id.0)),
                resume_entry,
                core_expr_without_yield(py_expr!("__dp_globals()")),
            ],
        ),
        BlockPyFunctionKind::Function => {
            unreachable!("plain functions do not use generator factories")
        }
    };

    block.set_term(BlockPyTerm::Return(factory_value));
    block.finish(None)
}

fn resume_param_spec(kind: BlockPyFunctionKind) -> ParamSpec {
    ParamSpec {
        params: resume_abi_params(kind)
            .iter()
            .map(|param| Param {
                name: param.name().to_string(),
                kind: ParamKind::PosOnly,
                has_default: false,
            })
            .collect(),
    }
}

fn fresh_resume_dispatch_label(
    blocks: &[CfgBlock<BlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>>],
    exhausted_label: &BlockPyLabel,
) -> BlockPyLabel {
    let base = "_dp_resume_dispatch";
    let existing = blocks
        .iter()
        .map(|block| block.label.as_str())
        .chain(std::iter::once(exhausted_label.as_str()))
        .collect::<HashSet<_>>();
    if !existing.contains(base) {
        return BlockPyLabel::from(base);
    }
    let mut next_id = 0usize;
    loop {
        let candidate = format!("{base}_{next_id}");
        if !existing.contains(candidate.as_str()) {
            return BlockPyLabel::from(candidate);
        }
        next_id += 1;
    }
}

#[derive(Clone)]
enum YieldSite {
    ExprYield(Option<CoreBlockPyExprWithYield>),
    AssignYield {
        target: ExprName,
        value: Option<CoreBlockPyExprWithYield>,
    },
    ReturnYield(Option<CoreBlockPyExprWithYield>),
    ExprYieldFrom(CoreBlockPyExprWithYield),
    AssignYieldFrom {
        target: ExprName,
        value: CoreBlockPyExprWithYield,
    },
    ReturnYieldFrom(CoreBlockPyExprWithYield),
}

fn stmt_yield_site(stmt: &BlockPyStmt<CoreBlockPyExprWithYield>) -> Option<YieldSite> {
    match stmt {
        BlockPyStmt::Expr(CoreBlockPyExprWithYield::Yield(yield_expr)) => {
            Some(YieldSite::ExprYield(yield_expr.value.as_deref().cloned()))
        }
        BlockPyStmt::Expr(CoreBlockPyExprWithYield::YieldFrom(yield_from)) => {
            Some(YieldSite::ExprYieldFrom((*yield_from.value).clone()))
        }
        BlockPyStmt::Assign(assign) => match &assign.value {
            CoreBlockPyExprWithYield::Yield(yield_expr) => Some(YieldSite::AssignYield {
                target: assign.target.clone(),
                value: yield_expr.value.as_deref().cloned(),
            }),
            CoreBlockPyExprWithYield::YieldFrom(yield_from) => Some(YieldSite::AssignYieldFrom {
                target: assign.target.clone(),
                value: (*yield_from.value).clone(),
            }),
            _ => None,
        },
        BlockPyStmt::Delete(_) | BlockPyStmt::If(_) | BlockPyStmt::Expr(_) => None,
    }
}

fn term_yield_site(term: &BlockPyTerm<CoreBlockPyExprWithYield>) -> Option<YieldSite> {
    match term {
        BlockPyTerm::Return(CoreBlockPyExprWithYield::Yield(yield_expr)) => {
            Some(YieldSite::ReturnYield(yield_expr.value.as_deref().cloned()))
        }
        BlockPyTerm::Return(CoreBlockPyExprWithYield::YieldFrom(yield_from)) => {
            Some(YieldSite::ReturnYieldFrom((*yield_from.value).clone()))
        }
        _ => None,
    }
}

fn lower_stmt_no_yield(
    stmt: BlockPyStmt<CoreBlockPyExprWithYield>,
) -> BlockPyStmt<CoreBlockPyExpr> {
    stmt.clone().try_into().unwrap_or_else(|_| {
        panic!(
            "generator lowering expected yield-like sites to be split before stmt conversion: {stmt:?}"
        )
    })
}

fn lower_term_no_yield(
    term: BlockPyTerm<CoreBlockPyExprWithYield>,
) -> BlockPyTerm<CoreBlockPyExpr> {
    term.clone().try_into().unwrap_or_else(|_| {
        panic!(
            "generator lowering expected yield-like sites to be split before term conversion: {term:?}"
        )
    })
}

fn yield_value_expr(value: Option<CoreBlockPyExprWithYield>) -> CoreBlockPyExpr {
    value
        .map(|value| {
            value
                .try_into()
                .unwrap_or_else(|_| panic!("yield payload unexpectedly contained nested yield"))
        })
        .unwrap_or_else(core_none)
}

fn completion_raise(
    kind: BlockPyFunctionKind,
    value: Option<CoreBlockPyExpr>,
) -> BlockPyTerm<CoreBlockPyExpr> {
    match kind {
        BlockPyFunctionKind::Generator | BlockPyFunctionKind::Coroutine => {
            let exc = if let Some(value) = value {
                core_call("StopIteration", vec![value])
            } else {
                core_call("StopIteration", Vec::new())
            };
            BlockPyTerm::Raise(BlockPyRaise { exc: Some(exc) })
        }
        BlockPyFunctionKind::AsyncGenerator => BlockPyTerm::Raise(BlockPyRaise {
            exc: Some(core_call("__dp_AsyncGenComplete", Vec::new())),
        }),
        BlockPyFunctionKind::Function => unreachable!(),
    }
}

fn push_completion_raise_block(
    state: &mut ResumeLoweringState,
    label: BlockPyLabel,
    mut body: Vec<BlockPyStmt<CoreBlockPyExpr>>,
    value: Option<CoreBlockPyExpr>,
    params: Vec<BlockParam>,
    exc_target: Option<String>,
) {
    body.push(BlockPyStmt::Assign(BlockPyAssign {
        target: expr_name("_dp_pc"),
        value: core_literal_int(0),
    }));
    body.push(BlockPyStmt::Assign(BlockPyAssign {
        target: expr_name("_dp_yieldfrom"),
        value: core_none(),
    }));
    let completion_label = state.fresh_label("resume_complete");
    state.push_block(
        BlockPyBlock {
            label,
            body,
            term: BlockPyTerm::Jump(completion_label.clone().into()),
            params: params.clone(),
            exc_edge: None,
        },
        exc_target,
    );
    state.push_block(
        BlockPyBlock {
            label: completion_label,
            body: Vec::new(),
            term: completion_raise(state.kind, value),
            params,
            exc_edge: None,
        },
        None,
    );
}

fn is_resume_exc_test() -> CoreBlockPyExpr {
    core_expr_without_yield(py_expr!("__dp_is_not(_dp_resume_exc, __dp_NO_DEFAULT)"))
}

fn is_send_none_test() -> CoreBlockPyExpr {
    core_expr_without_yield(py_expr!("__dp_is_(_dp_send_value, None)"))
}

fn is_name_none_test(name: &str) -> CoreBlockPyExpr {
    core_expr_without_yield(py_expr!("__dp_is_({name:id}, None)", name = name))
}

fn is_name_not_none_test(name: &str) -> CoreBlockPyExpr {
    core_expr_without_yield(py_expr!("__dp_is_not({name:id}, None)", name = name))
}

fn is_resume_generator_exit_test() -> CoreBlockPyExpr {
    core_expr_without_yield(py_expr!("isinstance(_dp_resume_exc, GeneratorExit)"))
}

fn resume_exc_raise_term() -> BlockPyTerm<CoreBlockPyExpr> {
    BlockPyTerm::Raise(BlockPyRaise {
        exc: Some(core_name("_dp_resume_exc")),
    })
}

fn stop_iteration_match_test() -> CoreBlockPyExpr {
    core_expr_without_yield(py_expr!(
        "__dp_exception_matches(__dp_current_exception(), StopIteration)"
    ))
}

fn current_exception_value_expr() -> CoreBlockPyExpr {
    core_expr_without_yield(py_expr!(
        "__dp_getattr(__dp_current_exception(), \"value\")"
    ))
}

struct ResumeLoweringState {
    kind: BlockPyFunctionKind,
    next_label_id: usize,
    next_resume_pc: usize,
    blocks: Vec<BlockPyBlock<CoreBlockPyExpr>>,
    exception_edges: HashMap<String, Option<String>>,
    resume_targets: Vec<(usize, BlockPyLabel)>,
    exhausted_label: BlockPyLabel,
}

impl ResumeLoweringState {
    fn new(kind: BlockPyFunctionKind) -> Self {
        Self {
            kind,
            next_label_id: 0,
            next_resume_pc: 2,
            blocks: Vec::new(),
            exception_edges: HashMap::new(),
            resume_targets: Vec::new(),
            exhausted_label: BlockPyLabel::from("_dp_resume_exhausted"),
        }
    }

    fn fresh_label(&mut self, base: &str) -> BlockPyLabel {
        let label = BlockPyLabel::from(format!("{base}_{}", self.next_label_id));
        self.next_label_id += 1;
        label
    }

    fn fresh_resume_target(&mut self, base: &str) -> (usize, BlockPyLabel) {
        let pc = self.next_resume_pc;
        self.next_resume_pc += 1;
        let label = self.fresh_label(base);
        self.resume_targets.push((pc, label.clone()));
        (pc, label)
    }

    fn fresh_temp(&mut self, base: &str) -> String {
        let name = format!("_dp_{base}_{}", self.next_label_id);
        self.next_label_id += 1;
        name
    }

    fn push_block(&mut self, block: BlockPyBlock<CoreBlockPyExpr>, exc_target: Option<String>) {
        self.exception_edges
            .insert(block.label.as_str().to_string(), exc_target);
        self.blocks.push(block);
    }
}

fn lower_resume_fragment(
    state: &mut ResumeLoweringState,
    label: BlockPyLabel,
    body: Vec<BlockPyStmt<CoreBlockPyExprWithYield>>,
    term: BlockPyTerm<CoreBlockPyExprWithYield>,
    params: Vec<BlockParam>,
    exc_target: Option<String>,
) {
    for (index, stmt) in body.iter().enumerate() {
        if let Some(site) = stmt_yield_site(stmt) {
            let mut prefix = body[..index]
                .iter()
                .cloned()
                .map(lower_stmt_no_yield)
                .collect::<Vec<_>>();
            emit_yield_site(
                state,
                label,
                &mut prefix,
                site,
                body[index + 1..].to_vec(),
                term,
                params,
                exc_target,
            );
            return;
        }
    }
    if let Some(site) = term_yield_site(&term) {
        let mut prefix = body
            .into_iter()
            .map(lower_stmt_no_yield)
            .collect::<Vec<_>>();
        emit_yield_site(
            state,
            label,
            &mut prefix,
            site,
            Vec::new(),
            BlockPyTerm::Return(core_none().into()),
            params,
            exc_target,
        );
        return;
    }

    let lowered_body = body
        .into_iter()
        .map(lower_stmt_no_yield)
        .collect::<Vec<_>>();
    match term {
        BlockPyTerm::Return(value) => {
            push_completion_raise_block(
                state,
                label,
                lowered_body,
                Some(value.try_into().unwrap_or_else(|_| {
                    panic!("generator lowering expected yield-free final return value")
                })),
                params,
                exc_target,
            );
        }
        other => {
            state.push_block(
                BlockPyBlock {
                    label,
                    body: lowered_body,
                    term: lower_term_no_yield(other),
                    params,
                    exc_edge: None,
                },
                exc_target,
            );
        }
    }
}

fn emit_yield_site(
    state: &mut ResumeLoweringState,
    label: BlockPyLabel,
    prefix: &mut Vec<BlockPyStmt<CoreBlockPyExpr>>,
    site: YieldSite,
    tail_body: Vec<BlockPyStmt<CoreBlockPyExprWithYield>>,
    tail_term: BlockPyTerm<CoreBlockPyExprWithYield>,
    params: Vec<BlockParam>,
    exc_target: Option<String>,
) {
    match site {
        YieldSite::ExprYield(value) => {
            let (resume_pc, resume_label) = state.fresh_resume_target("yield_resume");
            prefix.push(BlockPyStmt::Assign(BlockPyAssign {
                target: expr_name("_dp_pc"),
                value: core_literal_int(resume_pc),
            }));
            prefix.push(BlockPyStmt::Assign(BlockPyAssign {
                target: expr_name("_dp_yieldfrom"),
                value: core_none(),
            }));
            state.push_block(
                BlockPyBlock {
                    label,
                    body: std::mem::take(prefix),
                    term: BlockPyTerm::Return(yield_value_expr(value)),
                    params: params.clone(),
                    exc_edge: None,
                },
                exc_target.clone(),
            );
            emit_resume_after_yield(
                state,
                resume_label,
                None,
                tail_body,
                tail_term,
                params,
                exc_target,
            );
        }
        YieldSite::AssignYield { target, value } => {
            let (resume_pc, resume_label) = state.fresh_resume_target("yield_resume");
            prefix.push(BlockPyStmt::Assign(BlockPyAssign {
                target: expr_name("_dp_pc"),
                value: core_literal_int(resume_pc),
            }));
            prefix.push(BlockPyStmt::Assign(BlockPyAssign {
                target: expr_name("_dp_yieldfrom"),
                value: core_none(),
            }));
            state.push_block(
                BlockPyBlock {
                    label,
                    body: std::mem::take(prefix),
                    term: BlockPyTerm::Return(yield_value_expr(value)),
                    params: params.clone(),
                    exc_edge: None,
                },
                exc_target.clone(),
            );
            emit_resume_after_yield(
                state,
                resume_label,
                Some(target),
                tail_body,
                tail_term,
                params,
                exc_target,
            );
        }
        YieldSite::ReturnYield(value) => {
            let (resume_pc, resume_label) = state.fresh_resume_target("yield_return_resume");
            prefix.push(BlockPyStmt::Assign(BlockPyAssign {
                target: expr_name("_dp_pc"),
                value: core_literal_int(resume_pc),
            }));
            prefix.push(BlockPyStmt::Assign(BlockPyAssign {
                target: expr_name("_dp_yieldfrom"),
                value: core_none(),
            }));
            state.push_block(
                BlockPyBlock {
                    label,
                    body: std::mem::take(prefix),
                    term: BlockPyTerm::Return(yield_value_expr(value)),
                    params: params.clone(),
                    exc_edge: None,
                },
                exc_target.clone(),
            );
            emit_resume_after_yield(
                state,
                resume_label,
                None,
                Vec::new(),
                BlockPyTerm::Return(CoreBlockPyExprWithYield::Name(expr_name("_dp_send_value"))),
                params,
                exc_target,
            );
        }
        YieldSite::ExprYieldFrom(value) => emit_yield_from_site(
            state, label, prefix, value, None, tail_body, tail_term, params, exc_target,
        ),
        YieldSite::AssignYieldFrom { target, value } => emit_yield_from_site(
            state,
            label,
            prefix,
            value,
            Some(target),
            tail_body,
            tail_term,
            params,
            exc_target,
        ),
        YieldSite::ReturnYieldFrom(value) => emit_yield_from_site(
            state,
            label,
            prefix,
            value,
            None,
            Vec::new(),
            BlockPyTerm::Return(CoreBlockPyExprWithYield::Name(expr_name(
                "_dp_yield_from_value",
            ))),
            params,
            exc_target,
        ),
    }
}

fn emit_resume_after_yield(
    state: &mut ResumeLoweringState,
    resume_label: BlockPyLabel,
    assign_target: Option<ExprName>,
    mut tail_body: Vec<BlockPyStmt<CoreBlockPyExprWithYield>>,
    tail_term: BlockPyTerm<CoreBlockPyExprWithYield>,
    params: Vec<BlockParam>,
    exc_target: Option<String>,
) {
    let raise_label = state.fresh_label("yield_throw");
    let continue_label = state.fresh_label("yield_continue");
    state.push_block(
        BlockPyBlock {
            label: resume_label,
            body: Vec::new(),
            term: BlockPyTerm::IfTerm(BlockPyIfTerm {
                test: is_resume_exc_test(),
                then_label: raise_label.clone(),
                else_label: continue_label.clone(),
            }),
            params: params.clone(),
            exc_edge: None,
        },
        exc_target.clone(),
    );
    state.push_block(
        BlockPyBlock {
            label: raise_label,
            body: Vec::new(),
            term: resume_exc_raise_term(),
            params: params.clone(),
            exc_edge: None,
        },
        exc_target.clone(),
    );
    if let Some(target) = assign_target {
        tail_body.insert(
            0,
            BlockPyStmt::Assign(BlockPyAssign {
                target,
                value: CoreBlockPyExprWithYield::Name(expr_name("_dp_send_value")),
            }),
        );
    }
    lower_resume_fragment(
        state,
        continue_label,
        tail_body,
        tail_term,
        params,
        exc_target,
    );
}

#[allow(clippy::too_many_arguments)]
fn emit_yield_from_site(
    state: &mut ResumeLoweringState,
    label: BlockPyLabel,
    prefix: &mut Vec<BlockPyStmt<CoreBlockPyExpr>>,
    value: CoreBlockPyExprWithYield,
    assign_target: Option<ExprName>,
    mut tail_body: Vec<BlockPyStmt<CoreBlockPyExprWithYield>>,
    tail_term: BlockPyTerm<CoreBlockPyExprWithYield>,
    params: Vec<BlockParam>,
    exc_target: Option<String>,
) {
    let (delegate_pc, delegate_label) = state.fresh_resume_target("yield_from");
    let send_dispatch_label = state.fresh_label("yield_from_send_dispatch");
    let exc_dispatch_label = state.fresh_label("yield_from_exc_dispatch");
    let next_call_label = state.fresh_label("yield_from_next");
    let send_call_label = state.fresh_label("yield_from_send");
    let throw_lookup_label = state.fresh_label("yield_from_throw_lookup");
    let throw_call_label = state.fresh_label("yield_from_throw");
    let close_lookup_label = state.fresh_label("yield_from_close_lookup");
    let close_call_label = state.fresh_label("yield_from_close");
    let raise_resume_exc_label = state.fresh_label("yield_from_reraise");
    let call_except_label = state.fresh_label("yield_from_except");
    let stopiter_label = state.fresh_label("yield_from_stopiter");
    let non_stopiter_label = state.fresh_label("yield_from_non_stopiter");
    let value_expr: CoreBlockPyExpr = value
        .try_into()
        .unwrap_or_else(|_| panic!("yield from payload unexpectedly contained nested yield"));
    let yielded_value_name = state.fresh_temp("yield_from_value");
    let throw_name = state.fresh_temp("yield_from_throw");
    let close_name = state.fresh_temp("yield_from_close");
    let caught_exc_name = state.fresh_temp("yield_from_exc");
    prefix.push(BlockPyStmt::Assign(BlockPyAssign {
        target: expr_name("_dp_yieldfrom"),
        value: core_expr_without_yield(py_expr!(
            "iter({value:expr})",
            value = Expr::from(value_expr)
        )),
    }));
    prefix.push(BlockPyStmt::Assign(BlockPyAssign {
        target: expr_name("_dp_pc"),
        value: core_literal_int(delegate_pc),
    }));
    state.push_block(
        BlockPyBlock {
            label,
            body: std::mem::take(prefix),
            term: BlockPyTerm::Jump(delegate_label.clone().into()),
            params: params.clone(),
            exc_edge: None,
        },
        exc_target.clone(),
    );

    let yielded_label = state.fresh_label("yield_from_yielded");
    let done_label = state.fresh_label("yield_from_done");
    state.push_block(
        BlockPyBlock {
            label: delegate_label.clone(),
            body: Vec::new(),
            term: BlockPyTerm::IfTerm(BlockPyIfTerm {
                test: is_resume_exc_test(),
                then_label: exc_dispatch_label.clone(),
                else_label: send_dispatch_label.clone(),
            }),
            params: params.clone(),
            exc_edge: None,
        },
        exc_target.clone(),
    );
    state.push_block(
        BlockPyBlock {
            label: send_dispatch_label,
            body: Vec::new(),
            term: BlockPyTerm::IfTerm(BlockPyIfTerm {
                test: is_send_none_test(),
                then_label: next_call_label.clone(),
                else_label: send_call_label.clone(),
            }),
            params: params.clone(),
            exc_edge: None,
        },
        exc_target.clone(),
    );
    state.push_block(
        BlockPyBlock {
            label: next_call_label,
            body: vec![BlockPyStmt::Assign(BlockPyAssign {
                target: expr_name(yielded_value_name.as_str()),
                value: core_expr_without_yield(py_expr!("next(_dp_yieldfrom)")),
            })],
            term: BlockPyTerm::Jump(yielded_label.clone().into()),
            params: params.clone(),
            exc_edge: None,
        },
        Some(call_except_label.as_str().to_string()),
    );
    state.push_block(
        BlockPyBlock {
            label: send_call_label,
            body: vec![BlockPyStmt::Assign(BlockPyAssign {
                target: expr_name(yielded_value_name.as_str()),
                value: core_expr_without_yield(py_expr!("_dp_yieldfrom.send(_dp_send_value)")),
            })],
            term: BlockPyTerm::Jump(yielded_label.clone().into()),
            params: params.clone(),
            exc_edge: None,
        },
        Some(call_except_label.as_str().to_string()),
    );
    state.push_block(
        BlockPyBlock {
            label: exc_dispatch_label,
            body: Vec::new(),
            term: BlockPyTerm::IfTerm(BlockPyIfTerm {
                test: is_resume_generator_exit_test(),
                then_label: close_lookup_label.clone(),
                else_label: throw_lookup_label.clone(),
            }),
            params: params.clone(),
            exc_edge: None,
        },
        exc_target.clone(),
    );
    state.push_block(
        BlockPyBlock {
            label: close_lookup_label,
            body: vec![BlockPyStmt::Assign(BlockPyAssign {
                target: expr_name(close_name.as_str()),
                value: core_expr_without_yield(py_expr!("getattr(_dp_yieldfrom, \"close\", None)")),
            })],
            term: BlockPyTerm::IfTerm(BlockPyIfTerm {
                test: is_name_not_none_test(close_name.as_str()),
                then_label: close_call_label.clone(),
                else_label: raise_resume_exc_label.clone(),
            }),
            params: params.clone(),
            exc_edge: None,
        },
        exc_target.clone(),
    );
    state.push_block(
        BlockPyBlock {
            label: close_call_label,
            body: vec![BlockPyStmt::Expr(core_expr_without_yield(py_expr!(
                "{close:id}()",
                close = close_name.as_str(),
            )))],
            term: BlockPyTerm::Jump(raise_resume_exc_label.clone().into()),
            params: params.clone(),
            exc_edge: None,
        },
        exc_target.clone(),
    );
    state.push_block(
        BlockPyBlock {
            label: throw_lookup_label,
            body: vec![BlockPyStmt::Assign(BlockPyAssign {
                target: expr_name(throw_name.as_str()),
                value: core_expr_without_yield(py_expr!("getattr(_dp_yieldfrom, \"throw\", None)")),
            })],
            term: BlockPyTerm::IfTerm(BlockPyIfTerm {
                test: is_name_none_test(throw_name.as_str()),
                then_label: raise_resume_exc_label.clone(),
                else_label: throw_call_label.clone(),
            }),
            params: params.clone(),
            exc_edge: None,
        },
        exc_target.clone(),
    );
    state.push_block(
        BlockPyBlock {
            label: throw_call_label,
            body: vec![BlockPyStmt::Assign(BlockPyAssign {
                target: expr_name(yielded_value_name.as_str()),
                value: core_expr_without_yield(py_expr!(
                    "{throw_fn:id}(_dp_resume_exc)",
                    throw_fn = throw_name.as_str(),
                )),
            })],
            term: BlockPyTerm::Jump(yielded_label.clone().into()),
            params: params.clone(),
            exc_edge: None,
        },
        Some(call_except_label.as_str().to_string()),
    );
    let mut except_params = params.clone();
    if let Some(existing) = except_params
        .iter_mut()
        .find(|param| param.name == caught_exc_name)
    {
        existing.role = BlockParamRole::Exception;
    } else {
        for param in &mut except_params {
            if param.role == BlockParamRole::Exception {
                param.role = BlockParamRole::Local;
            }
        }
        except_params.push(BlockParam {
            name: caught_exc_name.clone(),
            role: BlockParamRole::Exception,
        });
    }
    state.push_block(
        BlockPyBlock {
            label: call_except_label.clone(),
            body: Vec::new(),
            term: BlockPyTerm::IfTerm(BlockPyIfTerm {
                test: stop_iteration_match_test(),
                then_label: stopiter_label.clone(),
                else_label: non_stopiter_label.clone(),
            }),
            params: except_params.clone(),
            exc_edge: None,
        },
        exc_target.clone(),
    );
    state.push_block(
        BlockPyBlock {
            label: stopiter_label,
            body: vec![BlockPyStmt::Assign(BlockPyAssign {
                target: expr_name(yielded_value_name.as_str()),
                value: current_exception_value_expr(),
            })],
            term: BlockPyTerm::Jump(done_label.clone().into()),
            params: except_params.clone(),
            exc_edge: None,
        },
        exc_target.clone(),
    );
    state.push_block(
        BlockPyBlock {
            label: non_stopiter_label,
            body: Vec::new(),
            term: BlockPyTerm::Raise(BlockPyRaise {
                exc: Some(core_name(caught_exc_name.as_str())),
            }),
            params: except_params,
            exc_edge: None,
        },
        exc_target.clone(),
    );
    state.push_block(
        BlockPyBlock {
            label: yielded_label,
            body: vec![BlockPyStmt::Assign(BlockPyAssign {
                target: expr_name("_dp_pc"),
                value: core_literal_int(delegate_pc),
            })],
            term: BlockPyTerm::Return(core_name(yielded_value_name.as_str())),
            params: params.clone(),
            exc_edge: None,
        },
        exc_target.clone(),
    );
    state.push_block(
        BlockPyBlock {
            label: raise_resume_exc_label,
            body: Vec::new(),
            term: resume_exc_raise_term(),
            params: params.clone(),
            exc_edge: None,
        },
        exc_target.clone(),
    );

    tail_body.insert(
        0,
        BlockPyStmt::Assign(BlockPyAssign {
            target: expr_name("_dp_yieldfrom"),
            value: CoreBlockPyExprWithYield::Name(expr_name("__dp_NONE")),
        }),
    );
    if let Some(target) = assign_target {
        tail_body.insert(
            1,
            BlockPyStmt::Assign(BlockPyAssign {
                target,
                value: CoreBlockPyExprWithYield::Name(expr_name(yielded_value_name.as_str())),
            }),
        );
    } else if matches!(tail_term, BlockPyTerm::Return(CoreBlockPyExprWithYield::Name(ref name)) if name.id.as_str() == "_dp_yield_from_value")
    {
        tail_body.insert(
            1,
            BlockPyStmt::Assign(BlockPyAssign {
                target: expr_name("_dp_yield_from_value"),
                value: CoreBlockPyExprWithYield::Name(expr_name(yielded_value_name.as_str())),
            }),
        );
    }
    lower_resume_fragment(state, done_label, tail_body, tail_term, params, exc_target);
}

fn lower_resume_blocks(
    callable: &BlockPyFunction<CoreBlockPyPassWithYield>,
) -> (
    Vec<CfgBlock<BlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>>>,
    HashMap<String, Option<String>>,
    String,
) {
    let block_params =
        recompute_lowered_block_params(callable, should_include_closure_storage_aliases(callable));
    let exception_edges = lowered_exception_edges(&callable.blocks);
    let (linear_blocks, _linear_params, linear_exception_edges) =
        linearize_structured_ifs(&callable.blocks, &block_params, &exception_edges);
    let linear_blocks = linear_blocks
        .into_iter()
        .map(make_eval_order_explicit_in_core_block_without_await)
        .map(|block| BlockPyBlock {
            label: block.label,
            body: block.body,
            term: block.term,
            params: block.params,
            exc_edge: None,
        })
        .collect::<Vec<_>>();
    let resume_entry_target = callable.entry_block().label_str().to_string();

    let mut state = ResumeLoweringState::new(callable.kind);
    state
        .resume_targets
        .push((1, BlockPyLabel::from(resume_entry_target.as_str())));

    let mut queue = linear_blocks
        .into_iter()
        .map(|block| {
            (
                block.label.clone(),
                block.body,
                block.term,
                block.params,
                linear_exception_edges
                    .get(block.label.as_str())
                    .cloned()
                    .unwrap_or(None),
            )
        })
        .collect::<VecDeque<_>>();
    while let Some((label, body, term, params, exc_target)) = queue.pop_front() {
        lower_resume_fragment(&mut state, label, body, term, params, exc_target);
    }

    let dispatch_label = fresh_resume_dispatch_label(&state.blocks, &state.exhausted_label);
    let targets_len = state
        .resume_targets
        .iter()
        .map(|(pc, _)| *pc)
        .max()
        .unwrap_or(1)
        + 1;
    let mut targets = vec![state.exhausted_label.clone(); targets_len];
    for (pc, label) in &state.resume_targets {
        targets[*pc] = label.clone();
    }

    let mut blocks = vec![BlockPyBlock {
        label: dispatch_label.clone(),
        body: Vec::new(),
        term: BlockPyTerm::BranchTable(BlockPyBranchTable {
            index: core_name("_dp_pc"),
            targets,
            default_label: state.exhausted_label.clone(),
        }),
        params: Vec::new(),
        exc_edge: None,
    }];
    blocks.append(&mut state.blocks);
    blocks.push(BlockPyBlock {
        label: state.exhausted_label.clone(),
        body: vec![
            BlockPyStmt::Assign(BlockPyAssign {
                target: expr_name("_dp_pc"),
                value: core_literal_int(0),
            }),
            BlockPyStmt::Assign(BlockPyAssign {
                target: expr_name("_dp_yieldfrom"),
                value: core_none(),
            }),
        ],
        term: completion_raise(state.kind, None),
        params: Vec::new(),
        exc_edge: None,
    });
    state
        .exception_edges
        .insert(dispatch_label.as_str().to_string(), None);
    state
        .exception_edges
        .insert(state.exhausted_label.as_str().to_string(), None);
    (
        attach_exception_edges_to_blocks(blocks, &state.exception_edges),
        state.exception_edges,
        dispatch_label.as_str().to_string(),
    )
}

pub(crate) fn lower_generator_like_function(
    callable: BlockPyFunction<CoreBlockPyPassWithYield>,
    resume_function_id: FunctionId,
) -> Vec<BlockPyFunction<CoreBlockPyPass>> {
    assert!(
        is_generator_like(callable.kind),
        "generator lowering only applies to generator-like callables"
    );
    let closure_layout = build_generator_closure_layout(&callable);
    let state_order = generator_state_order(&closure_layout, callable.kind);
    let (resume_blocks, _resume_exception_edges, _resume_entry_label) =
        lower_resume_blocks(&callable);
    let resume_blocks = sync_resume_state_blocks(resume_blocks, &closure_layout);

    let factory_block = build_factory_block(
        callable.function_id,
        resume_function_id,
        &state_order,
        &closure_layout,
        callable.kind,
    );

    let visible_function = BlockPyFunction {
        function_id: callable.function_id,
        names: callable.names.clone(),
        kind: callable.kind,
        params: callable.params.clone(),
        blocks: attach_exception_edges_to_blocks(
            vec![factory_block.clone()],
            &HashMap::from([(factory_block.label.as_str().to_string(), None)]),
        ),
        doc: callable.doc.clone(),
        closure_layout: Some(closure_layout.clone()),
        facts: callable.facts.clone(),
    };

    let resume_params = resume_param_spec(callable.kind);
    let resume_function = BlockPyFunction {
        function_id: resume_function_id,
        names: FunctionName::new(
            format!("{}_resume", callable.names.bind_name),
            "_dp_resume",
            callable.names.display_name.clone(),
            callable.names.qualname.clone(),
        ),
        kind: BlockPyFunctionKind::Function,
        params: resume_params.clone(),
        blocks: resume_blocks.clone(),
        doc: None,
        closure_layout: Some(closure_layout.clone()),
        facts: callable.facts,
    };

    vec![visible_function, resume_function]
}

#[cfg(test)]
mod tests {
    use super::{build_blockpy_closure_layout, closure_value_name_for_state, resume_closure_names};
    use crate::block_py::{
        BlockPyCfgBlockBuilder, BlockPyTerm, ClosureInit, ClosureLayout, ClosureSlot, FunctionId,
    };
    use crate::passes::ruff_to_blockpy::lower_stmts_to_blockpy_stmts;
    use crate::{py_expr, py_stmt};
    use ruff_python_ast::Expr;
    use std::collections::HashSet;

    fn blockpy_make_dp_tuple(items: Vec<Expr>) -> Expr {
        let Expr::Call(mut call) = py_expr!("__dp_tuple()") else {
            panic!("expected call expression for __dp_tuple");
        };
        call.arguments.args = items.into();
        Expr::Call(call)
    }

    fn closure_backed_generator_init_expr(slot: &ClosureSlot) -> Expr {
        match slot.init {
            ClosureInit::InheritedCapture => {
                panic!("inherited captures do not allocate new cells in outer factories")
            }
            ClosureInit::Parameter => {
                py_expr!("{name:id}", name = slot.logical_name.as_str())
            }
            ClosureInit::DeletedSentinel => py_expr!("__dp_DELETED"),
            ClosureInit::RuntimePcUnstarted => py_expr!("1"),
            ClosureInit::RuntimeNone => py_expr!("None"),
            ClosureInit::Deferred => py_expr!("None"),
        }
    }

    fn build_closure_backed_generator_factory_block(
        factory_label: &str,
        visible_function_id: FunctionId,
        resume_function_id: FunctionId,
        resume_state_order: &[String],
        layout: &ClosureLayout,
        is_coroutine: bool,
        is_async_generator: bool,
    ) -> crate::block_py::BlockPyBlock<Expr> {
        let mut body = Vec::new();

        for slot in layout.cellvars.iter().chain(layout.runtime_cells.iter()) {
            let stmt = py_stmt!(
                "{cell:id} = __dp_make_cell({init:expr})",
                cell = slot.storage_name.as_str(),
                init = closure_backed_generator_init_expr(slot),
            );
            let lowered = lower_stmts_to_blockpy_stmts::<Expr>(&[stmt])
                .unwrap_or_else(|err| panic!("failed to lower generator factory cell init: {err}"));
            assert!(lowered.term.is_none());
            body.extend(lowered.body);
        }

        let closure_names = resume_closure_names(layout, resume_state_order);
        let closure_values = blockpy_make_dp_tuple(
            closure_names
                .iter()
                .map(|state_name| {
                    py_expr!(
                        "{name:id}",
                        name = closure_value_name_for_state(layout, state_name.as_str()).as_str()
                    )
                })
                .collect(),
        );

        let resume_entry = py_expr!(
            "__dp_def_hidden_resume_fn({function_id:literal}, {closure_names:expr}, {closure_values:expr}, __dp_globals(), async_gen={async_gen:expr})",
            function_id = resume_function_id.0,
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
                "__dp_make_closure_async_generator({function_id:literal}, {resume:expr}, __dp_globals())",
                function_id = visible_function_id.0,
                resume = resume_entry,
            )
        } else {
            py_expr!(
                "__dp_make_closure_generator({function_id:literal}, {resume:expr}, __dp_globals())",
                function_id = visible_function_id.0,
                resume = resume_entry,
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

        let mut block = BlockPyCfgBlockBuilder::new(factory_label.into());
        block.extend(body);
        block.set_term(BlockPyTerm::Return(return_value.into()));
        block.finish(None)
    }

    #[test]
    fn build_blockpy_closure_layout_classifies_capture_local_and_runtime_cells() {
        let layout = build_blockpy_closure_layout(
            &["arg".to_string()],
            &[
                "_dp_self".to_string(),
                "arg".to_string(),
                "captured".to_string(),
                "_dp_yieldfrom".to_string(),
                "_dp_pc".to_string(),
                "_dp_try_exc_0".to_string(),
            ],
            &["captured".to_string()],
            &HashSet::from(["_dp_try_exc_0".to_string()]),
        );

        assert_eq!(
            layout
                .freevars
                .iter()
                .map(|slot| (slot.logical_name.as_str(), slot.storage_name.as_str()))
                .collect::<Vec<_>>(),
            vec![("captured", "_dp_cell_captured")]
        );
        assert_eq!(
            layout
                .cellvars
                .iter()
                .map(|slot| (
                    slot.logical_name.as_str(),
                    slot.storage_name.as_str(),
                    &slot.init
                ))
                .collect::<Vec<_>>(),
            vec![
                ("arg", "_dp_cell_arg", &ClosureInit::Parameter),
                (
                    "_dp_try_exc_0",
                    "_dp_cell__dp_try_exc_0",
                    &ClosureInit::DeletedSentinel
                ),
            ]
        );
        assert_eq!(
            layout
                .runtime_cells
                .iter()
                .map(|slot| (
                    slot.logical_name.as_str(),
                    slot.storage_name.as_str(),
                    &slot.init
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    "_dp_yieldfrom",
                    "_dp_cell__dp_yieldfrom",
                    &ClosureInit::RuntimeNone
                ),
                (
                    "_dp_pc",
                    "_dp_cell__dp_pc",
                    &ClosureInit::RuntimePcUnstarted
                ),
            ]
        );
    }

    #[test]
    fn builds_closure_backed_generator_factory_block() {
        let layout = ClosureLayout {
            freevars: vec![ClosureSlot {
                logical_name: "captured".to_string(),
                storage_name: "_dp_cell_captured".to_string(),
                init: ClosureInit::InheritedCapture,
            }],
            cellvars: vec![ClosureSlot {
                logical_name: "x".to_string(),
                storage_name: "_dp_cell_x".to_string(),
                init: ClosureInit::Parameter,
            }],
            runtime_cells: vec![ClosureSlot {
                logical_name: "_dp_pc".to_string(),
                storage_name: "_dp_cell__dp_pc".to_string(),
                init: ClosureInit::RuntimePcUnstarted,
            }],
        };

        let block = build_closure_backed_generator_factory_block(
            "_dp_bb_demo_factory",
            FunctionId(1),
            FunctionId(0),
            &[
                "_dp_self".to_string(),
                "_dp_send_value".to_string(),
                "_dp_resume_exc".to_string(),
                "_dp_cell_captured".to_string(),
                "_dp_cell_x".to_string(),
                "_dp_cell__dp_pc".to_string(),
            ],
            &layout,
            false,
            false,
        );

        assert_eq!(block.label.as_str(), "_dp_bb_demo_factory");
        assert!(matches!(block.term, BlockPyTerm::Return(_)));
    }

    #[test]
    fn resume_closure_names_include_storage_aliases_for_cell_backed_state() {
        let layout = ClosureLayout {
            freevars: vec![ClosureSlot {
                logical_name: "captured".to_string(),
                storage_name: "_dp_cell_captured".to_string(),
                init: ClosureInit::InheritedCapture,
            }],
            cellvars: vec![ClosureSlot {
                logical_name: "total".to_string(),
                storage_name: "_dp_cell_total".to_string(),
                init: ClosureInit::Deferred,
            }],
            runtime_cells: vec![ClosureSlot {
                logical_name: "_dp_pc".to_string(),
                storage_name: "_dp_cell__dp_pc".to_string(),
                init: ClosureInit::RuntimePcUnstarted,
            }],
        };

        assert_eq!(
            resume_closure_names(
                &layout,
                &[
                    "_dp_self".to_string(),
                    "_dp_send_value".to_string(),
                    "_dp_resume_exc".to_string(),
                    "_dp_cell_captured".to_string(),
                    "total".to_string(),
                    "_dp_pc".to_string(),
                ],
            ),
            vec![
                "_dp_cell_captured".to_string(),
                "total".to_string(),
                "_dp_pc".to_string(),
                "_dp_cell_total".to_string(),
                "_dp_cell__dp_pc".to_string(),
            ]
        );
    }
}
