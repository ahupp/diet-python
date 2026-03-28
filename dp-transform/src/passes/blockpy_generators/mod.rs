use crate::block_py::param_specs::{Param, ParamKind, ParamSpec};
use crate::block_py::state::collect_state_vars;
use crate::block_py::{
    core_operation_expr, core_positional_call_expr_with_meta, BbStmt, BlockParam, BlockParamRole,
    BlockPyAssign, BlockPyBindingKind, BlockPyBlock, BlockPyBranchTable,
    BlockPyCallableSemanticInfo, BlockPyCellBindingKind, BlockPyCfgBlockBuilder, BlockPyFunction,
    BlockPyFunctionKind, BlockPyIfTerm, BlockPyLabel, BlockPyRaise, BlockPyStmt, BlockPyTerm,
    CellRef, CfgBlock, ClosureInit, ClosureLayout, ClosureSlot, CoreBlockPyExpr,
    CoreBlockPyExprWithAwaitAndYield, CoreBlockPyExprWithYield, FunctionId, FunctionName,
    IntoBlockPyStmt, ModuleNameGen, Operation,
};
use crate::passes::ast_to_ast::expr_utils::make_dp_tuple;
use crate::passes::ast_to_ast::scope_helpers::is_internal_symbol;
use crate::passes::ruff_to_blockpy::{
    attach_exception_edges_to_blocks, lowered_exception_edges, recompute_lowered_block_params,
    should_include_closure_storage_aliases,
};
use crate::passes::{CoreBlockPyPass, CoreBlockPyPassWithYield};
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr, ExprName};
use std::collections::HashSet;
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ResumeAbiParam {
    SelfValue,
    SendValue,
    ResumeExc,
    TransportSent,
}

impl ResumeAbiParam {
    fn name(self) -> &'static str {
        match self {
            Self::SelfValue => "_dp_self",
            Self::SendValue => "_dp_send_value",
            Self::ResumeExc => "_dp_resume_exc",
            Self::TransportSent => "_dp_transport_sent",
        }
    }
}

const GENERATOR_RESUME_ABI_PARAMS: [ResumeAbiParam; 3] = [
    ResumeAbiParam::SelfValue,
    ResumeAbiParam::SendValue,
    ResumeAbiParam::ResumeExc,
];

const ASYNC_GENERATOR_RESUME_ABI_PARAMS: [ResumeAbiParam; 4] = [
    ResumeAbiParam::SelfValue,
    ResumeAbiParam::SendValue,
    ResumeAbiParam::ResumeExc,
    ResumeAbiParam::TransportSent,
];

fn resume_abi_params(kind: BlockPyFunctionKind) -> &'static [ResumeAbiParam] {
    match kind {
        BlockPyFunctionKind::Function => &[],
        BlockPyFunctionKind::Coroutine | BlockPyFunctionKind::Generator => {
            &GENERATOR_RESUME_ABI_PARAMS
        }
        BlockPyFunctionKind::AsyncGenerator => &ASYNC_GENERATOR_RESUME_ABI_PARAMS,
    }
}

fn generator_state_logical_name(semantic: &BlockPyCallableSemanticInfo, name: &str) -> String {
    semantic
        .logical_name_for_cell_storage(name)
        .unwrap_or_else(|| name.to_string())
}

fn generator_state_storage_name(semantic: &BlockPyCallableSemanticInfo, name: &str) -> String {
    let logical_name = generator_state_logical_name(semantic, name);
    semantic.cell_storage_name(logical_name.as_str())
}

fn runtime_init(name: &str) -> Option<ClosureInit> {
    match name {
        "_dp_pc" => Some(ClosureInit::RuntimePcUnstarted),
        name if name.starts_with("_dp_try_abrupt_kind_") => {
            Some(ClosureInit::RuntimeAbruptKindFallthrough)
        }
        "_dp_yieldfrom" => Some(ClosureInit::RuntimeNone),
        _ => None,
    }
}

pub(crate) fn build_blockpy_closure_layout(
    semantic: &BlockPyCallableSemanticInfo,
    param_names: &[String],
    state_vars: &[String],
    capture_names: &[String],
    injected_exception_names: &HashSet<String>,
) -> ClosureLayout {
    let capture_names = capture_names.iter().cloned().collect::<HashSet<_>>();
    let mut seen_storage_names = HashSet::new();

    let mut freevars = Vec::new();
    let mut cellvars = Vec::new();
    let mut runtime_cells = Vec::new();

    for name in state_vars {
        let logical_name = generator_state_logical_name(semantic, name.as_str());
        let storage_name = generator_state_storage_name(semantic, name.as_str());
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
        if capture_names.contains(name.as_str())
            || capture_names.contains(logical_name.as_str())
            || capture_names.contains(storage_name.as_str())
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

fn core_string(value: &str) -> CoreBlockPyExpr {
    CoreBlockPyExpr::Literal(crate::block_py::CoreBlockPyLiteral::StringLiteral(
        crate::block_py::CoreStringLiteral {
            node_index: ast::AtomicNodeIndex::default(),
            range: Default::default(),
            value: value.to_string(),
        },
    ))
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

fn core_cell_ref(logical_name: &str) -> CoreBlockPyExpr {
    core_operation_expr(Operation::CellRef(CellRef {
        node_index: ast::AtomicNodeIndex::default(),
        range: Default::default(),
        arg0: core_string(logical_name),
    }))
}

fn is_generator_like(kind: BlockPyFunctionKind) -> bool {
    matches!(
        kind,
        BlockPyFunctionKind::Generator
            | BlockPyFunctionKind::Coroutine
            | BlockPyFunctionKind::AsyncGenerator
    )
}

fn injected_exception_names<S>(
    blocks: &[CfgBlock<S, BlockPyTerm<CoreBlockPyExprWithYield>>],
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
    let param_names = callable.params.names();
    let local_cell_slot_names = callable.semantic.local_cell_storage_names();
    let mut local_cell_slots = local_cell_slot_names.iter().cloned().collect::<Vec<_>>();
    local_cell_slots.sort();
    let mut capture_names = callable
        .closure_layout
        .as_ref()
        .map(|layout| {
            layout
                .freevars
                .iter()
                .map(|slot| slot.storage_name.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    capture_names.sort();
    capture_names.dedup();

    let mut state_vars = collect_state_vars(&param_names, &callable.blocks);
    for capture_name in &capture_names {
        if !state_vars.iter().any(|existing| existing == capture_name) {
            state_vars.push(capture_name.clone());
        }
    }
    for block in &callable.blocks {
        if let Some(exc_param) = block.exception_param() {
            if !state_vars.iter().any(|existing| existing == exc_param) {
                state_vars.push(exc_param.to_string());
            }
        }
    }
    for slot in local_cell_slots {
        let logical_name = callable
            .semantic
            .logical_name_for_cell_storage(slot.as_str())
            .unwrap_or_else(|| slot.clone());
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
        &callable.semantic,
        &param_names,
        &state_vars,
        &capture_names,
        &injected_exception_names(&callable.blocks),
    )
}

fn persistent_generator_state_order(layout: &ClosureLayout) -> Vec<String> {
    let mut order = Vec::new();
    order.extend(layout.freevars.iter().map(|slot| slot.logical_name.clone()));
    order.extend(layout.cellvars.iter().map(|slot| slot.logical_name.clone()));
    order.extend(
        layout
            .runtime_cells
            .iter()
            .map(|slot| slot.logical_name.clone()),
    );
    order
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResumeClosureBindings {
    runtime_state_bindings: Vec<(String, String)>,
}

impl ResumeClosureBindings {
    fn all_bindings(&self) -> impl Iterator<Item = &(String, String)> {
        self.runtime_state_bindings.iter()
    }
}

fn resume_state_uses_standard_name_binding(name: &str) -> bool {
    !name.starts_with("_dp_cell_")
}

fn augment_resume_semantic_for_standard_name_binding(
    semantic: &mut BlockPyCallableSemanticInfo,
    closure_bindings: &ResumeClosureBindings,
) {
    for (name, _) in &closure_bindings.runtime_state_bindings {
        if resume_state_uses_standard_name_binding(name.as_str()) {
            semantic.insert_binding(
                name.clone(),
                BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture),
                is_internal_symbol(name.as_str()),
                Some(name.clone()),
            );
        }
    }
}

fn resume_closure_value_name(layout: &ClosureLayout, name: &str) -> String {
    layout
        .freevars
        .iter()
        .chain(layout.cellvars.iter())
        .chain(layout.runtime_cells.iter())
        .find(|slot| slot.logical_name == name || slot.storage_name == name)
        .map(|slot| slot.logical_name.clone())
        .unwrap_or_else(|| name.to_string())
}

fn is_resume_closure_state_name(layout: &ClosureLayout, name: &str) -> bool {
    layout
        .freevars
        .iter()
        .chain(layout.cellvars.iter())
        .chain(layout.runtime_cells.iter())
        .any(|slot| slot.logical_name == name || slot.storage_name == name)
}

fn resume_closure_bindings(
    layout: &ClosureLayout,
    persistent_state_names: &[String],
) -> ResumeClosureBindings {
    let runtime_state_bindings = persistent_state_names
        .iter()
        .filter(|name| is_resume_closure_state_name(layout, name.as_str()))
        .map(|name| {
            (
                name.clone(),
                resume_closure_value_name(layout, name.as_str()),
            )
        })
        .collect::<Vec<_>>();
    ResumeClosureBindings {
        runtime_state_bindings,
    }
}

fn build_resume_closure_layout(
    visible_layout: &ClosureLayout,
    closure_bindings: &ResumeClosureBindings,
) -> ClosureLayout {
    let freevars = closure_bindings
        .runtime_state_bindings
        .iter()
        .map(|(name, _)| {
            let slot = visible_layout
                .freevars
                .iter()
                .chain(visible_layout.cellvars.iter())
                .chain(visible_layout.runtime_cells.iter())
                .find(|slot| slot.logical_name == *name || slot.storage_name == *name)
                .unwrap_or_else(|| {
                    panic!("missing visible closure slot for resume state binding {name}")
                });
            ClosureSlot {
                logical_name: slot.logical_name.clone(),
                storage_name: slot.storage_name.clone(),
                init: ClosureInit::InheritedCapture,
            }
        })
        .collect();
    ClosureLayout {
        freevars,
        cellvars: Vec::new(),
        runtime_cells: Vec::new(),
    }
}

fn generator_resume_declared_params(
    kind: BlockPyFunctionKind,
    params: &[BlockParam],
) -> Vec<BlockParam> {
    let kept_indices = generator_resume_declared_param_indices(kind, params);
    params
        .iter()
        .enumerate()
        .filter(|(index, _)| kept_indices.contains(index))
        .map(|(_, param)| param.clone())
        .collect()
}

fn generator_resume_declared_param_indices(
    kind: BlockPyFunctionKind,
    params: &[BlockParam],
) -> Vec<usize> {
    let resume_abi_names = resume_abi_params(kind)
        .iter()
        .map(|param| param.name())
        .collect::<HashSet<_>>();
    params
        .iter()
        .enumerate()
        .filter(|(_, param)| {
            param.role == BlockParamRole::Exception
                || resume_abi_names.contains(param.name.as_str())
        })
        .map(|(index, _)| index)
        .collect()
}

fn build_factory_block(
    visible_function_id: FunctionId,
    resume_function_id: FunctionId,
    closure_bindings: &ResumeClosureBindings,
    kind: BlockPyFunctionKind,
) -> BlockPyBlock<CoreBlockPyExpr> {
    let mut block = BlockPyCfgBlockBuilder::new(BlockPyLabel::from("_dp_factory_entry"));

    let all_bindings = closure_bindings.all_bindings().cloned().collect::<Vec<_>>();
    let captures = all_bindings
        .iter()
        .map(|(name, value_name)| {
            make_dp_tuple(vec![
                py_expr!("{value:literal}", value = name.as_str()),
                Expr::from(core_cell_ref(value_name.as_str())),
            ])
        })
        .collect::<Vec<_>>();

    let resume_entry = core_expr_without_yield(py_expr!(
        "__dp_make_function({function_id:literal}, \"function\", {captures:expr}, __dp_tuple(), __dp_globals(), None)",
        function_id = resume_function_id.0,
        captures = make_dp_tuple(captures),
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

fn flatten_core_blocks(
    blocks: Vec<CfgBlock<BlockPyStmt<CoreBlockPyExpr>, BlockPyTerm<CoreBlockPyExpr>>>,
) -> Vec<CfgBlock<BbStmt<CoreBlockPyExpr, ExprName>, BlockPyTerm<CoreBlockPyExpr>>> {
    blocks
        .into_iter()
        .map(|block| CfgBlock {
            label: block.label,
            body: block.body.into_iter().map(BbStmt::from).collect(),
            term: block.term,
            params: block.params,
            exc_edge: block.exc_edge,
        })
        .collect()
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
    target_arg_indices: HashMap<String, Vec<usize>>,
    resume_targets: Vec<(usize, BlockPyLabel)>,
    exhausted_label: BlockPyLabel,
}

impl ResumeLoweringState {
    fn new(kind: BlockPyFunctionKind, target_arg_indices: HashMap<String, Vec<usize>>) -> Self {
        Self {
            kind,
            next_label_id: 0,
            next_resume_pc: 2,
            blocks: Vec::new(),
            exception_edges: HashMap::new(),
            target_arg_indices,
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

    fn prune_term_target_args(&self, term: &mut BlockPyTerm<CoreBlockPyExpr>) {
        let BlockPyTerm::Jump(edge) = term else {
            return;
        };
        let Some(indices) = self.target_arg_indices.get(edge.target.as_str()) else {
            return;
        };
        if edge.args.is_empty() {
            return;
        }
        edge.args = indices
            .iter()
            .filter_map(|index| edge.args.get(*index).cloned())
            .collect();
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
            let mut lowered_term = lower_term_no_yield(other);
            state.prune_term_target_args(&mut lowered_term);
            state.push_block(
                BlockPyBlock {
                    label,
                    body: lowered_body,
                    term: lowered_term,
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
    let linear_exception_edges = lowered_exception_edges(&callable.blocks);
    let declared_param_indices_by_label = callable
        .blocks
        .iter()
        .map(|block| {
            (
                block.label.as_str().to_string(),
                generator_resume_declared_param_indices(callable.kind, &block.params),
            )
        })
        .collect::<HashMap<_, _>>();
    let linear_blocks = callable
        .blocks
        .iter()
        .cloned()
        .map(|block| BlockPyBlock {
            label: block.label,
            body: block
                .body
                .into_iter()
                .map(|stmt| stmt.into_stmt())
                .collect(),
            term: block.term,
            params: block.params,
            exc_edge: None,
        })
        .collect::<Vec<_>>();
    let resume_entry_target = callable.entry_block().label.clone();

    let mut state = ResumeLoweringState::new(callable.kind, declared_param_indices_by_label);
    state.resume_targets.push((1, resume_entry_target));

    let mut queue = linear_blocks
        .into_iter()
        .map(|block| {
            (
                block.label.clone(),
                block.body,
                block.term,
                generator_resume_declared_params(callable.kind, &block.params),
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

fn ordered_resume_binding_names(
    callable: &BlockPyFunction<CoreBlockPyPassWithYield>,
    persistent_state_order: &[String],
) -> Vec<String> {
    let mut seen = HashSet::new();
    persistent_state_order
        .iter()
        .cloned()
        .chain(
            recompute_lowered_block_params(
                callable,
                should_include_closure_storage_aliases(callable),
            )
            .into_values()
            .flatten(),
        )
        .filter(|name| seen.insert(name.clone()))
        .collect()
}

pub(crate) fn lower_generator_like_function(
    callable: BlockPyFunction<CoreBlockPyPassWithYield>,
    module_name_gen: &mut ModuleNameGen,
) -> Vec<BlockPyFunction<CoreBlockPyPass>> {
    assert!(
        is_generator_like(callable.kind),
        "generator lowering only applies to generator-like callables"
    );
    let resume_name_gen = module_name_gen.next_function_name_gen();
    let resume_function_id = resume_name_gen.function_id();
    let closure_layout = build_generator_closure_layout(&callable);
    let persistent_state_order = persistent_generator_state_order(&closure_layout);
    let resume_binding_names = ordered_resume_binding_names(&callable, &persistent_state_order);
    let (resume_blocks, _resume_exception_edges, _resume_entry_label) =
        lower_resume_blocks(&callable);
    let closure_bindings = resume_closure_bindings(&closure_layout, &resume_binding_names);
    let resume_closure_layout = build_resume_closure_layout(&closure_layout, &closure_bindings);

    let factory_block = build_factory_block(
        callable.function_id,
        resume_function_id,
        &closure_bindings,
        callable.kind,
    );

    let mut resume_semantic = callable.semantic.clone();
    augment_resume_semantic_for_standard_name_binding(&mut resume_semantic, &closure_bindings);

    let BlockPyFunction {
        function_id,
        name_gen,
        names,
        kind,
        params,
        doc,
        semantic,
        ..
    } = callable;

    let visible_function = BlockPyFunction {
        function_id,
        name_gen,
        names: names.clone(),
        kind,
        params: params.clone(),
        blocks: flatten_core_blocks(attach_exception_edges_to_blocks(
            vec![factory_block.clone()],
            &HashMap::from([(factory_block.label.as_str().to_string(), None)]),
        )),
        doc,
        closure_layout: Some(closure_layout.clone()),
        semantic: semantic.clone(),
    };

    let resume_params = resume_param_spec(kind);
    let resume_names = FunctionName::new(
        format!("{}_resume", names.bind_name),
        "_dp_resume",
        names.display_name.clone(),
        names.qualname.clone(),
    );
    let resume_function = BlockPyFunction {
        function_id: resume_function_id,
        name_gen: resume_name_gen,
        names: resume_names,
        kind: BlockPyFunctionKind::Function,
        params: resume_params.clone(),
        blocks: flatten_core_blocks(resume_blocks.clone()),
        doc: None,
        closure_layout: Some(resume_closure_layout),
        semantic: resume_semantic,
    };

    vec![visible_function, resume_function]
}

#[cfg(test)]
mod test;
