use crate::block_py::param_specs::{Param, ParamKind, ParamSpec};
use crate::block_py::state::collect_state_vars;
use crate::block_py::{
    compute_storage_layout_from_semantics, core_call_expr_with_meta, core_operation_expr,
    core_runtime_name_expr_with_meta, core_runtime_positional_call_expr_with_meta,
    try_lower_core_expr_without_await, try_lower_core_expr_without_yield, BlockArg, BlockParam,
    BlockParamRole, BlockPyAssign, BlockPyBindingKind, BlockPyBranchTable,
    BlockPyCallableSemanticInfo, BlockPyCellBindingKind, BlockPyCfgBlockBuilder, BlockPyEdge,
    BlockPyFunction, BlockPyFunctionKind, BlockPyIfTerm, BlockPyLabel, BlockPyNameLike,
    BlockPyRaise, BlockPyStmt, BlockPyTerm, CellRefForName, CfgBlock, ClosureInit, ClosureSlot,
    CoreBlockPyCallArg, CoreBlockPyExpr, CoreBlockPyExprWithAwaitAndYield,
    CoreBlockPyExprWithYield, CoreBlockPyKeywordArg, ExprTryMap, FunctionId, FunctionName,
    ImplicitNoneExpr, MakeFunction, Meta, ModuleNameGen, PassStmt, StorageLayout, UnresolvedName,
    WithMeta,
};
use crate::passes::ast_to_ast::scope_helpers::is_internal_symbol;
use crate::passes::ruff_to_blockpy::{attach_exception_edges_to_blocks, lowered_exception_edges};
use crate::passes::{CoreBlockPyPass, CoreBlockPyPassWithYield};
use crate::py_expr;
use ruff_python_ast::{self as ast, Expr};
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

type LinearYieldStmt = PassStmt<CoreBlockPyPassWithYield>;
type LinearCoreStmt = PassStmt<CoreBlockPyPass>;
type LinearYieldBlock = CfgBlock<LinearYieldStmt, BlockPyTerm<CoreBlockPyExprWithYield>>;
type LinearCoreBlock = CfgBlock<LinearCoreStmt, BlockPyTerm<CoreBlockPyExpr>>;
type BlockPyBlock = LinearCoreBlock;

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
        "_dp_throw_context" => Some(ClosureInit::RuntimeNone),
        _ => None,
    }
}

pub(crate) fn build_blockpy_storage_layout(
    semantic: &BlockPyCallableSemanticInfo,
    param_names: &[String],
    state_vars: &[String],
    capture_names: &[String],
    injected_exception_names: &HashSet<String>,
) -> StorageLayout {
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

    StorageLayout {
        freevars,
        cellvars,
        runtime_cells,
        stack_slots: Vec::new(),
    }
}

fn expr_name(id: &str) -> UnresolvedName {
    let Expr::Name(expr) = py_expr!("{id:id}", id = id) else {
        unreachable!();
    };
    expr.into()
}

fn core_expr_without_yield(expr: Expr) -> CoreBlockPyExpr {
    let core = CoreBlockPyExprWithAwaitAndYield::from(expr);
    let core_without_await = try_lower_core_expr_without_await(core)
        .unwrap_or_else(|_| panic!("generator helper expression unexpectedly contained await"));
    try_lower_core_expr_without_yield(core_without_await)
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

fn core_string_literal(value: &str) -> CoreBlockPyExpr {
    core_expr_without_yield(py_expr!("{value:literal}", value = value))
}

fn core_call(func_name: &str, args: Vec<CoreBlockPyExpr>) -> CoreBlockPyExpr {
    core_runtime_positional_call_expr_with_meta(
        func_name,
        ast::AtomicNodeIndex::default(),
        Default::default(),
        args,
    )
}

fn core_call_expr(
    func: CoreBlockPyExpr,
    args: Vec<CoreBlockPyExpr>,
    keywords: Vec<(&str, CoreBlockPyExpr)>,
) -> CoreBlockPyExpr {
    core_call_expr_with_meta(
        func,
        ast::AtomicNodeIndex::default(),
        Default::default(),
        args.into_iter()
            .map(CoreBlockPyCallArg::Positional)
            .collect(),
        keywords
            .into_iter()
            .map(|(arg, value)| CoreBlockPyKeywordArg::Named {
                arg: ast::Identifier::new(arg, Default::default()),
                value,
            })
            .collect(),
    )
}

fn core_runtime_attr(attr: &str) -> CoreBlockPyExpr {
    core_runtime_name_expr_with_meta(attr, ast::AtomicNodeIndex::default(), Default::default())
}

fn core_cell_ref(logical_name: &str) -> CoreBlockPyExpr {
    core_operation_expr(CellRefForName::new(logical_name.to_string()).with_meta(Meta::synthetic()))
}

fn core_generator_code(async_gen: bool, name: &str, qualname: &str) -> CoreBlockPyExpr {
    let template_attr = if async_gen {
        "code_template_async_gen"
    } else {
        "code_template_gen"
    };
    core_expr_without_yield(py_expr!(
        "__soac__.{template_attr:id}.__code__.replace(co_name={name:literal}, co_qualname={qualname:literal})",
        template_attr = template_attr,
        name = name,
        qualname = qualname,
    ))
}

fn core_make_function(
    function_id: FunctionId,
    kind: BlockPyFunctionKind,
    param_defaults: CoreBlockPyExpr,
    annotate_fn: CoreBlockPyExpr,
) -> CoreBlockPyExpr {
    core_operation_expr(
        MakeFunction::new(
            function_id,
            kind,
            Box::new(param_defaults),
            Box::new(annotate_fn),
        )
        .with_meta(Meta::synthetic()),
    )
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

fn build_generator_storage_layout(
    callable: &BlockPyFunction<CoreBlockPyPassWithYield>,
) -> StorageLayout {
    let param_names = callable.params.names();
    let semantic_layout =
        compute_storage_layout_from_semantics(callable).unwrap_or(StorageLayout {
            freevars: Vec::new(),
            cellvars: Vec::new(),
            runtime_cells: Vec::new(),
            stack_slots: Vec::new(),
        });
    let capture_names = semantic_layout
        .freevars
        .iter()
        .map(|slot| slot.logical_name.clone())
        .collect::<Vec<_>>();
    let local_cell_slots = semantic_layout
        .cellvars
        .iter()
        .map(|slot| slot.storage_name.clone())
        .collect::<Vec<_>>();

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
    for runtime_name in ["_dp_pc", "_dp_yieldfrom", "_dp_throw_context"] {
        if !state_vars.iter().any(|existing| existing == runtime_name) {
            state_vars.push(runtime_name.to_string());
        }
    }

    build_blockpy_storage_layout(
        &callable.semantic,
        &param_names,
        &state_vars,
        &capture_names,
        &injected_exception_names(&callable.blocks),
    )
}

fn persistent_generator_state_order(layout: &StorageLayout) -> Vec<String> {
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

impl ResumeClosureBindings {}

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

fn resume_closure_value_name(layout: &StorageLayout, name: &str) -> String {
    layout
        .freevars
        .iter()
        .chain(layout.cellvars.iter())
        .chain(layout.runtime_cells.iter())
        .find(|slot| slot.logical_name == name || slot.storage_name == name)
        .map(|slot| slot.logical_name.clone())
        .unwrap_or_else(|| name.to_string())
}

fn is_resume_closure_state_name(layout: &StorageLayout, name: &str) -> bool {
    layout
        .freevars
        .iter()
        .chain(layout.cellvars.iter())
        .chain(layout.runtime_cells.iter())
        .any(|slot| slot.logical_name == name || slot.storage_name == name)
}

fn resume_closure_bindings(
    layout: &StorageLayout,
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

fn build_resume_storage_layout(
    visible_layout: &StorageLayout,
    closure_bindings: &ResumeClosureBindings,
) -> StorageLayout {
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
    StorageLayout {
        freevars,
        cellvars: Vec::new(),
        runtime_cells: Vec::new(),
        stack_slots: Vec::new(),
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
                || param.role == BlockParamRole::AbruptKind
                || param.role == BlockParamRole::AbruptPayload
                || resume_abi_names.contains(param.name.as_str())
        })
        .map(|(index, _)| index)
        .collect()
}

fn build_factory_block(
    visible_names: &FunctionName,
    resume_function_id: FunctionId,
    kind: BlockPyFunctionKind,
) -> LinearCoreBlock {
    let mut block: BlockPyCfgBlockBuilder<LinearCoreStmt, BlockPyTerm<CoreBlockPyExpr>> =
        BlockPyCfgBlockBuilder::new(BlockPyLabel::from_index(0));

    let resume_entry = core_make_function(
        resume_function_id,
        BlockPyFunctionKind::Function,
        core_call("tuple_values", Vec::new()),
        core_none(),
    );
    let generator = match kind {
        BlockPyFunctionKind::Generator | BlockPyFunctionKind::Coroutine => core_call_expr(
            core_runtime_attr("ClosureGenerator"),
            Vec::new(),
            vec![
                ("resume", resume_entry),
                (
                    "name",
                    core_string_literal(visible_names.display_name.as_str()),
                ),
                (
                    "qualname",
                    core_string_literal(visible_names.qualname.as_str()),
                ),
                (
                    "code",
                    core_generator_code(
                        false,
                        visible_names.display_name.as_str(),
                        visible_names.qualname.as_str(),
                    ),
                ),
                ("yieldfrom_cell", core_cell_ref("_dp_yieldfrom")),
                ("throw_context_cell", core_cell_ref("_dp_throw_context")),
            ],
        ),
        BlockPyFunctionKind::AsyncGenerator => core_call_expr(
            core_runtime_attr("ClosureAsyncGenerator"),
            Vec::new(),
            vec![
                ("resume", resume_entry),
                (
                    "name",
                    core_string_literal(visible_names.display_name.as_str()),
                ),
                (
                    "qualname",
                    core_string_literal(visible_names.qualname.as_str()),
                ),
                (
                    "code",
                    core_generator_code(
                        true,
                        visible_names.display_name.as_str(),
                        visible_names.qualname.as_str(),
                    ),
                ),
                ("yieldfrom_cell", core_cell_ref("_dp_yieldfrom")),
                ("throw_context_cell", core_cell_ref("_dp_throw_context")),
            ],
        ),
        BlockPyFunctionKind::Function => {
            unreachable!("plain functions do not use generator factories")
        }
    };
    let factory_value = match kind {
        BlockPyFunctionKind::Coroutine => {
            core_call_expr(core_runtime_attr("Coroutine"), vec![generator], Vec::new())
        }
        BlockPyFunctionKind::Generator | BlockPyFunctionKind::AsyncGenerator => generator,
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
    blocks: &[LinearCoreBlock],
    exhausted_label: &BlockPyLabel,
) -> BlockPyLabel {
    let max_index = blocks
        .iter()
        .map(|block| block.label.index())
        .chain(std::iter::once(exhausted_label.index()))
        .max()
        .unwrap_or(0);
    BlockPyLabel::from_index(max_index + 1)
}

#[derive(Clone)]
enum YieldSite {
    ExprYield(Option<CoreBlockPyExprWithYield>),
    AssignYield {
        target: UnresolvedName,
        value: Option<CoreBlockPyExprWithYield>,
    },
    ReturnYield(Option<CoreBlockPyExprWithYield>),
    ExprYieldFrom(CoreBlockPyExprWithYield),
    AssignYieldFrom {
        target: UnresolvedName,
        value: CoreBlockPyExprWithYield,
    },
    ReturnYieldFrom(CoreBlockPyExprWithYield),
}

fn stmt_yield_site(stmt: &LinearYieldStmt) -> Option<YieldSite> {
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
        BlockPyStmt::Delete(_) | BlockPyStmt::Expr(_) => None,
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

fn lower_stmt_no_yield(stmt: LinearYieldStmt) -> LinearCoreStmt {
    ExprTryMap::<CoreBlockPyPassWithYield, CoreBlockPyPass, CoreBlockPyExprWithYield>::without_yield()
        .try_map_stmt(stmt.clone())
        .unwrap_or_else(|_| {
            panic!(
                "generator lowering expected yield-like sites to be split before stmt conversion: {stmt:?}"
            )
        })
}

fn lower_term_no_yield(
    term: BlockPyTerm<CoreBlockPyExprWithYield>,
) -> BlockPyTerm<CoreBlockPyExpr> {
    ExprTryMap::<CoreBlockPyPassWithYield, CoreBlockPyPass, CoreBlockPyExprWithYield>::without_yield()
        .try_map_term(term.clone())
        .unwrap_or_else(|_| {
        panic!(
            "generator lowering expected yield-like sites to be split before term conversion: {term:?}"
        )
    })
}

fn yield_value_expr(value: Option<CoreBlockPyExprWithYield>) -> CoreBlockPyExpr {
    value
        .map(|value| {
            try_lower_core_expr_without_yield(value)
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
            exc: Some(core_call("AsyncGenComplete", Vec::new())),
        }),
        BlockPyFunctionKind::Function => unreachable!(),
    }
}

fn push_completion_raise_block(
    state: &mut ResumeLoweringState,
    label: BlockPyLabel,
    body: Vec<LinearCoreStmt>,
    value: Option<CoreBlockPyExpr>,
    params: Vec<BlockParam>,
    exc_target: Option<BlockPyLabel>,
) {
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

fn explicit_jump_args_for_params(params: &[BlockParam]) -> Vec<BlockArg> {
    params
        .iter()
        .map(|param| BlockArg::Name(param.name.clone()))
        .collect()
}

fn is_resume_exc_test() -> CoreBlockPyExpr {
    core_operation_expr(
        crate::block_py::operation::UnaryOp::new(
            crate::block_py::operation::UnaryOpKind::Not,
            Box::new(core_operation_expr(
                crate::block_py::operation::BinOp::new(
                    crate::block_py::operation::BinOpKind::Is,
                    Box::new(core_name("_dp_resume_exc")),
                    Box::new(core_runtime_name_expr_with_meta(
                        "NO_DEFAULT",
                        ast::AtomicNodeIndex::default(),
                        Default::default(),
                    )),
                )
                .with_meta(Meta::synthetic()),
            )),
        )
        .with_meta(Meta::synthetic()),
    )
}

fn is_send_none_test() -> CoreBlockPyExpr {
    core_operation_expr(
        crate::block_py::operation::BinOp::new(
            crate::block_py::operation::BinOpKind::Is,
            Box::new(core_name("_dp_send_value")),
            Box::new(core_none()),
        )
        .with_meta(Meta::synthetic()),
    )
}

fn is_name_none_test(name: &str) -> CoreBlockPyExpr {
    core_operation_expr(
        crate::block_py::operation::BinOp::new(
            crate::block_py::operation::BinOpKind::Is,
            Box::new(core_name(name)),
            Box::new(core_none()),
        )
        .with_meta(Meta::synthetic()),
    )
}

fn is_name_not_none_test(name: &str) -> CoreBlockPyExpr {
    core_expr_without_yield(py_expr!("{name:id} is not None", name = name))
}

fn is_resume_generator_exit_test() -> CoreBlockPyExpr {
    core_expr_without_yield(py_expr!("isinstance(_dp_resume_exc, GeneratorExit)"))
}

fn resume_exc_raise_term() -> BlockPyTerm<CoreBlockPyExpr> {
    BlockPyTerm::Raise(BlockPyRaise {
        exc: Some(core_name("_dp_resume_exc")),
    })
}

fn stop_iteration_match_test(exc_name: &str) -> CoreBlockPyExpr {
    core_expr_without_yield(py_expr!(
        "__soac__.exception_matches({exc_name:id}, StopIteration)",
        exc_name = exc_name,
    ))
}

fn current_exception_value_expr(exc_name: &str) -> CoreBlockPyExpr {
    core_expr_without_yield(py_expr!("{exc_name:id}.value", exc_name = exc_name))
}

struct ResumeLoweringState {
    kind: BlockPyFunctionKind,
    next_label_id: usize,
    next_resume_pc: usize,
    blocks: Vec<LinearCoreBlock>,
    exception_edges: HashMap<BlockPyLabel, Option<BlockPyLabel>>,
    target_arg_indices: HashMap<BlockPyLabel, Vec<usize>>,
    resume_targets: Vec<(usize, BlockPyLabel)>,
    exhausted_label: BlockPyLabel,
}

impl ResumeLoweringState {
    fn new(
        kind: BlockPyFunctionKind,
        target_arg_indices: HashMap<BlockPyLabel, Vec<usize>>,
    ) -> Self {
        let next_label_id = target_arg_indices
            .keys()
            .map(|label| label.index())
            .max()
            .unwrap_or(0)
            + 1;
        let exhausted_label = BlockPyLabel::from_index(next_label_id);
        Self {
            kind,
            next_label_id: next_label_id + 1,
            next_resume_pc: 2,
            blocks: Vec::new(),
            exception_edges: HashMap::new(),
            target_arg_indices,
            resume_targets: Vec::new(),
            exhausted_label,
        }
    }

    fn fresh_label(&mut self, base: &str) -> BlockPyLabel {
        let _ = base;
        let label = BlockPyLabel::from_index(self.next_label_id);
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

    fn push_block(&mut self, mut block: LinearCoreBlock, exc_target: Option<BlockPyLabel>) {
        let active_exception = block
            .params
            .iter()
            .find(|param| param.role == BlockParamRole::Exception)
            .map(|param| core_name(param.name.as_str()))
            .unwrap_or_else(core_none);
        block.body.insert(
            0,
            BlockPyStmt::Assign(BlockPyAssign {
                target: expr_name("_dp_throw_context"),
                value: active_exception,
            }),
        );
        self.exception_edges.insert(block.label.clone(), exc_target);
        self.blocks.push(block);
    }

    fn prune_term_target_args(&self, term: &mut BlockPyTerm<CoreBlockPyExpr>) {
        let BlockPyTerm::Jump(edge) = term else {
            return;
        };
        let Some(indices) = self.target_arg_indices.get(&edge.target) else {
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
    body: Vec<LinearYieldStmt>,
    term: BlockPyTerm<CoreBlockPyExprWithYield>,
    params: Vec<BlockParam>,
    exc_target: Option<BlockPyLabel>,
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
            BlockPyTerm::Return(CoreBlockPyExprWithYield::implicit_none_expr()),
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
                Some(
                    try_lower_core_expr_without_yield(value).unwrap_or_else(|_| {
                        panic!("generator lowering expected yield-free final return value")
                    }),
                ),
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
    prefix: &mut Vec<LinearCoreStmt>,
    site: YieldSite,
    tail_body: Vec<LinearYieldStmt>,
    tail_term: BlockPyTerm<CoreBlockPyExprWithYield>,
    params: Vec<BlockParam>,
    exc_target: Option<BlockPyLabel>,
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
    assign_target: Option<UnresolvedName>,
    mut tail_body: Vec<LinearYieldStmt>,
    tail_term: BlockPyTerm<CoreBlockPyExprWithYield>,
    params: Vec<BlockParam>,
    exc_target: Option<BlockPyLabel>,
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
    prefix: &mut Vec<LinearCoreStmt>,
    value: CoreBlockPyExprWithYield,
    assign_target: Option<UnresolvedName>,
    mut tail_body: Vec<LinearYieldStmt>,
    tail_term: BlockPyTerm<CoreBlockPyExprWithYield>,
    params: Vec<BlockParam>,
    exc_target: Option<BlockPyLabel>,
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
    let value_expr = try_lower_core_expr_without_yield(value)
        .unwrap_or_else(|_| panic!("yield from payload unexpectedly contained nested yield"));
    let yielded_value_name = state.fresh_temp("yield_from_value");
    let throw_name = state.fresh_temp("yield_from_throw");
    let close_name = state.fresh_temp("yield_from_close");
    let caught_exc_name = state.fresh_temp("yield_from_exc");
    prefix.push(BlockPyStmt::Assign(BlockPyAssign {
        target: expr_name("_dp_yieldfrom"),
        value: core_call("iter", vec![value_expr]),
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
        Some(call_except_label.clone()),
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
        Some(call_except_label.clone()),
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
        Some(call_except_label.clone()),
    );
    let mut except_params = params.clone();
    except_params
        .retain(|param| param.role != BlockParamRole::Exception || param.name == caught_exc_name);
    if let Some(existing) = except_params
        .iter_mut()
        .find(|param| param.name == caught_exc_name)
    {
        existing.role = BlockParamRole::Exception;
    } else {
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
                test: stop_iteration_match_test(caught_exc_name.as_str()),
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
                value: current_exception_value_expr(caught_exc_name.as_str()),
            })],
            term: BlockPyTerm::Jump(BlockPyEdge::with_args(
                done_label.clone(),
                explicit_jump_args_for_params(&params),
            )),
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
            value: CoreBlockPyExprWithYield::implicit_none_expr(),
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
    } else if matches!(tail_term, BlockPyTerm::Return(CoreBlockPyExprWithYield::Name(ref name)) if name.id_str() == "_dp_yield_from_value")
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
    Vec<LinearCoreBlock>,
    HashMap<BlockPyLabel, Option<BlockPyLabel>>,
    BlockPyLabel,
) {
    let linear_exception_edges = lowered_exception_edges(&callable.blocks);
    let declared_param_indices_by_label = callable
        .blocks
        .iter()
        .map(|block| {
            (
                block.label.clone(),
                generator_resume_declared_param_indices(callable.kind, &block.params),
            )
        })
        .collect::<HashMap<_, _>>();
    let linear_blocks = callable
        .blocks
        .iter()
        .cloned()
        .map(|block| LinearYieldBlock {
            label: block.label,
            body: block.body,
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
                    .get(&block.label)
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

    let mut blocks = vec![LinearCoreBlock {
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
    blocks.push(LinearCoreBlock {
        label: state.exhausted_label.clone(),
        body: Vec::new(),
        term: completion_raise(state.kind, None),
        params: Vec::new(),
        exc_edge: None,
    });
    state.exception_edges.insert(dispatch_label.clone(), None);
    state
        .exception_edges
        .insert(state.exhausted_label.clone(), None);
    (
        attach_exception_edges_to_blocks(blocks, &state.exception_edges),
        state.exception_edges,
        dispatch_label,
    )
}

fn ordered_resume_binding_names(
    _callable: &BlockPyFunction<CoreBlockPyPassWithYield>,
    persistent_state_order: &[String],
) -> Vec<String> {
    let mut seen = HashSet::new();
    persistent_state_order
        .iter()
        .cloned()
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
    let storage_layout = build_generator_storage_layout(&callable);
    let persistent_state_order = persistent_generator_state_order(&storage_layout);
    let resume_binding_names = ordered_resume_binding_names(&callable, &persistent_state_order);
    let (resume_blocks, _resume_exception_edges, _resume_entry_label) =
        lower_resume_blocks(&callable);
    let closure_bindings = resume_closure_bindings(&storage_layout, &resume_binding_names);
    let resume_storage_layout = build_resume_storage_layout(&storage_layout, &closure_bindings);

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

    let factory_block = build_factory_block(&names, resume_function_id, kind);

    let mut resume_semantic = semantic.clone();
    augment_resume_semantic_for_standard_name_binding(&mut resume_semantic, &closure_bindings);

    let visible_function = BlockPyFunction {
        function_id,
        name_gen,
        names: names.clone(),
        kind,
        params: params.clone(),
        blocks: attach_exception_edges_to_blocks(
            vec![factory_block.clone()],
            &HashMap::from([(factory_block.label.clone(), None)]),
        ),
        doc,
        storage_layout: Some(storage_layout.clone()),
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
        blocks: resume_blocks.clone(),
        doc: None,
        storage_layout: Some(resume_storage_layout),
        semantic: resume_semantic,
    };

    vec![visible_function, resume_function]
}

#[cfg(test)]
mod test;
