use crate::block_py::cfg::RelabelBlockTargets;
use crate::block_py::param_specs::{Param, ParamKind, ParamSpec};
use crate::block_py::{
    compute_storage_layout_from_semantics, core_call_expr_with_meta,
    core_runtime_name_expr_with_meta, core_runtime_positional_call_expr_with_meta, Block, BlockArg,
    BlockBuilder, BlockEdge, BlockLabel, BlockParam, BlockParamRole, BlockPyBindingKind,
    BlockPyCallableSemanticInfo, BlockPyCellBindingKind, BlockPyFunction, BlockPyModuleTryMap,
    BlockPyNameLike, BlockPySemanticExprNode, BlockTerm, CellRefForName, ClosureInit, ClosureSlot,
    CoreBlockPyCallArg, CoreBlockPyExpr, CoreBlockPyExprWithAwaitAndYield,
    CoreBlockPyExprWithYield, CoreBlockPyKeywordArg, ErrOnAwait, ErrOnYield, FunctionId,
    FunctionKind, FunctionName, FunctionNameGen, ImplicitNoneExpr, Instr, Load, MakeFunction,
    ModuleNameGen, StorageLayout, Store, TermBranchTable, TermIf, TermRaise, TryMapExpr,
    UnresolvedName,
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

type LinearYieldStmt = CoreBlockPyExprWithYield;
type LinearCoreStmt = CoreBlockPyExpr;
type LinearYieldBlock = Block<LinearYieldStmt, CoreBlockPyExprWithYield>;
type LinearCoreBlock = Block<LinearCoreStmt, CoreBlockPyExpr>;
type BlockPyBlock = LinearCoreBlock;

fn resume_abi_params(kind: FunctionKind) -> &'static [ResumeAbiParam] {
    match kind {
        FunctionKind::Function => &[],
        FunctionKind::Coroutine | FunctionKind::Generator => &GENERATOR_RESUME_ABI_PARAMS,
        FunctionKind::AsyncGenerator => &ASYNC_GENERATOR_RESUME_ABI_PARAMS,
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
    let core_without_await = ErrOnAwait
        .try_map_expr(core)
        .unwrap_or_else(|_| panic!("generator helper expression unexpectedly contained await"));
    ErrOnYield
        .try_map_expr(core_without_await)
        .unwrap_or_else(|_| panic!("generator helper expression unexpectedly contained yield"))
}

fn core_name(name: &str) -> CoreBlockPyExpr {
    core_expr_without_yield(py_expr!("{name:id}", name = name))
}

fn internal_store_stmt<E>(target: &str, value: E) -> E
where
    E: Instr<Name = UnresolvedName> + From<Store<E>>,
{
    unresolved_store_stmt(expr_name(target), value)
}

fn unresolved_store_stmt<E>(target: UnresolvedName, value: E) -> E
where
    E: Instr<Name = UnresolvedName> + From<Store<E>>,
{
    Store::new(target, Box::new(value)).into()
}

fn unresolved_load_expr<E>(name: UnresolvedName) -> E
where
    E: Instr<Name = UnresolvedName> + From<Load<E>>,
{
    Load::new(name).into()
}

fn collect_state_vars<E>(param_names: &[String], blocks: &[Block<E, E>]) -> Vec<String>
where
    E: BlockPySemanticExprNode + Instr,
{
    let mut state = param_names.to_vec();
    for block in blocks {
        for param_name in block
            .exception_param()
            .into_iter()
            .chain(block.param_names())
        {
            if !state.iter().any(|existing| existing == param_name) {
                state.push(param_name.to_string());
            }
        }
        for stmt in &block.body {
            for name in assigned_names_in_linear_stmt(stmt) {
                if !state.iter().any(|existing| existing == &name) {
                    state.push(name);
                }
            }
        }
        for name in assigned_names_in_term(&block.term) {
            if !state.iter().any(|existing| existing == &name) {
                state.push(name);
            }
        }
    }
    state
}

fn assigned_names_in_linear_stmt<E>(stmt: &E) -> HashSet<String>
where
    E: BlockPySemanticExprNode + Instr,
{
    let mut names = HashSet::new();
    collect_named_expr_target_names(stmt, &mut names);
    names
}

fn assigned_names_in_term<E>(term: &BlockTerm<E>) -> HashSet<String>
where
    E: BlockPySemanticExprNode + Instr,
{
    match term {
        BlockTerm::Jump(_) => HashSet::new(),
        BlockTerm::IfTerm(TermIf { test, .. }) => {
            let mut names = HashSet::new();
            collect_named_expr_target_names(test, &mut names);
            names
        }
        BlockTerm::BranchTable(TermBranchTable { index, .. }) => {
            let mut names = HashSet::new();
            collect_named_expr_target_names(index, &mut names);
            names
        }
        BlockTerm::Return(value) => {
            let mut names = HashSet::new();
            collect_named_expr_target_names(value, &mut names);
            names
        }
        BlockTerm::Raise(TermRaise { exc }) => {
            let mut names = HashSet::new();
            if let Some(exc) = exc {
                collect_named_expr_target_names(exc, &mut names);
            }
            names
        }
    }
}

fn collect_named_expr_target_names<E>(expr: &E, names: &mut HashSet<String>)
where
    E: BlockPySemanticExprNode,
{
    expr.walk_root_defined_names(&mut |name| {
        names.insert(name.to_string());
    });
    expr.walk(&mut |child| {
        collect_named_expr_target_names(child, names);
    });
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
    CellRefForName::new(logical_name.to_string()).into()
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
    kind: FunctionKind,
    param_defaults: CoreBlockPyExpr,
    annotate_fn: CoreBlockPyExpr,
) -> CoreBlockPyExpr {
    MakeFunction::new(
        function_id,
        kind,
        Box::new(param_defaults),
        Box::new(annotate_fn),
    )
    .into()
}

fn is_generator_like(kind: FunctionKind) -> bool {
    matches!(
        kind,
        FunctionKind::Generator | FunctionKind::Coroutine | FunctionKind::AsyncGenerator
    )
}

fn injected_exception_names<S>(blocks: &[Block<S, CoreBlockPyExprWithYield>]) -> HashSet<String> {
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
    for (name, source_name) in &closure_bindings.runtime_state_bindings {
        if resume_state_uses_standard_name_binding(name.as_str()) {
            semantic.insert_binding_with_cell_names(
                name.clone(),
                BlockPyBindingKind::Cell(BlockPyCellBindingKind::Capture),
                is_internal_symbol(name.as_str()),
                Some(name.clone()),
                Some(source_name.clone()),
            );
        }
    }
}

fn resume_closure_bindings(
    semantic: &BlockPyCallableSemanticInfo,
    persistent_logical_names: &[String],
) -> ResumeClosureBindings {
    let runtime_state_bindings = persistent_logical_names
        .iter()
        .map(|logical_name| {
            (
                logical_name.clone(),
                semantic.cell_capture_source_name(logical_name.as_str()),
            )
        })
        .collect::<Vec<_>>();
    ResumeClosureBindings {
        runtime_state_bindings,
    }
}

fn generator_resume_declared_params(kind: FunctionKind, params: &[BlockParam]) -> Vec<BlockParam> {
    let kept_indices = generator_resume_declared_param_indices(kind, params);
    params
        .iter()
        .enumerate()
        .filter(|(index, _)| kept_indices.contains(index))
        .map(|(_, param)| param.clone())
        .collect()
}

fn generator_resume_declared_param_indices(
    kind: FunctionKind,
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
    kind: FunctionKind,
) -> LinearCoreBlock {
    let resume_entry = core_make_function(
        resume_function_id,
        FunctionKind::Function,
        core_call("tuple_values", Vec::new()),
        core_none(),
    );
    let generator = match kind {
        FunctionKind::Generator | FunctionKind::Coroutine => core_call_expr(
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
        FunctionKind::AsyncGenerator => core_call_expr(
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
        FunctionKind::Function => {
            unreachable!("plain functions do not use generator factories")
        }
    };
    let factory_value = match kind {
        FunctionKind::Coroutine => {
            core_call_expr(core_runtime_attr("Coroutine"), vec![generator], Vec::new())
        }
        FunctionKind::Generator | FunctionKind::AsyncGenerator => generator,
        FunctionKind::Function => {
            unreachable!("plain functions do not use generator factories")
        }
    };

    Block::from_builder(
        BlockLabel::from_index(0),
        BlockBuilder::with_term(Vec::new(), Some(BlockTerm::Return(factory_value))),
        Vec::new(),
        None,
        None,
    )
}

fn resume_param_spec(kind: FunctionKind) -> ParamSpec {
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

fn explicit_yield_value(value: &CoreBlockPyExprWithYield) -> Option<CoreBlockPyExprWithYield> {
    (!CoreBlockPyExprWithYield::is_implicit_none_expr(value)).then(|| value.clone())
}

fn stmt_yield_site(stmt: &LinearYieldStmt) -> Option<YieldSite> {
    match stmt {
        CoreBlockPyExprWithYield::Yield(yield_expr) => Some(YieldSite::ExprYield(
            explicit_yield_value(&yield_expr.value),
        )),
        CoreBlockPyExprWithYield::YieldFrom(yield_from) => {
            Some(YieldSite::ExprYieldFrom((*yield_from.value).clone()))
        }
        CoreBlockPyExprWithYield::Store(store) => match store.value.as_ref() {
            CoreBlockPyExprWithYield::Yield(yield_expr) => Some(YieldSite::AssignYield {
                target: store.name.clone(),
                value: explicit_yield_value(&yield_expr.value),
            }),
            CoreBlockPyExprWithYield::YieldFrom(yield_from) => Some(YieldSite::AssignYieldFrom {
                target: store.name.clone(),
                value: (*yield_from.value).clone(),
            }),
            _ => None,
        },
        _ => None,
    }
}

fn term_yield_site(term: &BlockTerm<CoreBlockPyExprWithYield>) -> Option<YieldSite> {
    match term {
        BlockTerm::Return(CoreBlockPyExprWithYield::Yield(yield_expr)) => Some(
            YieldSite::ReturnYield(explicit_yield_value(&yield_expr.value)),
        ),
        BlockTerm::Return(CoreBlockPyExprWithYield::YieldFrom(yield_from)) => {
            Some(YieldSite::ReturnYieldFrom((*yield_from.value).clone()))
        }
        _ => None,
    }
}

fn lower_stmt_no_yield(stmt: LinearYieldStmt) -> LinearCoreStmt {
    let mut mapper = ErrOnYield;
    mapper.try_map_expr(stmt.clone()).unwrap_or_else(|_| {
            panic!(
                "generator lowering expected yield-like sites to be split before stmt conversion: {stmt:?}"
            )
        })
}

fn lower_term_no_yield(term: BlockTerm<CoreBlockPyExprWithYield>) -> BlockTerm<CoreBlockPyExpr> {
    let mut mapper = ErrOnYield;
    mapper.try_map_term(term.clone()).unwrap_or_else(|_| {
        panic!(
            "generator lowering expected yield-like sites to be split before term conversion: {term:?}"
        )
    })
}

fn yield_value_expr(value: Option<CoreBlockPyExprWithYield>) -> CoreBlockPyExpr {
    value
        .map(|value| {
            ErrOnYield
                .try_map_expr(value)
                .unwrap_or_else(|_| panic!("yield payload unexpectedly contained nested yield"))
        })
        .unwrap_or_else(core_none)
}

fn completion_raise(
    kind: FunctionKind,
    value: Option<CoreBlockPyExpr>,
) -> BlockTerm<CoreBlockPyExpr> {
    match kind {
        FunctionKind::Generator | FunctionKind::Coroutine => {
            let exc = if let Some(value) = value {
                core_call("StopIteration", vec![value])
            } else {
                core_call("StopIteration", Vec::new())
            };
            BlockTerm::Raise(TermRaise { exc: Some(exc) })
        }
        FunctionKind::AsyncGenerator => BlockTerm::Raise(TermRaise {
            exc: Some(core_call("AsyncGenComplete", Vec::new())),
        }),
        FunctionKind::Function => unreachable!(),
    }
}

fn push_completion_raise_block(
    state: &mut ResumeLoweringState,
    label: BlockLabel,
    body: Vec<LinearCoreStmt>,
    value: Option<CoreBlockPyExpr>,
    params: Vec<BlockParam>,
    exc_target: Option<BlockLabel>,
) {
    let completion_label = state.fresh_label("resume_complete");
    state.push_block(
        BlockPyBlock {
            label,
            body,
            term: BlockTerm::Jump(BlockEdge::new(completion_label.clone())),
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
    crate::block_py::operation::UnaryOp::new(
        crate::block_py::operation::UnaryOpKind::Not,
        Box::new(
            crate::block_py::operation::BinOp::new(
                crate::block_py::operation::BinOpKind::Is,
                Box::new(core_name("_dp_resume_exc")),
                Box::new(core_runtime_name_expr_with_meta(
                    "NO_DEFAULT",
                    ast::AtomicNodeIndex::default(),
                    Default::default(),
                )),
            )
            .into(),
        ),
    )
    .into()
}

fn is_send_none_test() -> CoreBlockPyExpr {
    crate::block_py::operation::BinOp::new(
        crate::block_py::operation::BinOpKind::Is,
        Box::new(core_name("_dp_send_value")),
        Box::new(core_none()),
    )
    .into()
}

fn is_name_none_test(name: &str) -> CoreBlockPyExpr {
    crate::block_py::operation::BinOp::new(
        crate::block_py::operation::BinOpKind::Is,
        Box::new(core_name(name)),
        Box::new(core_none()),
    )
    .into()
}

fn is_name_not_none_test(name: &str) -> CoreBlockPyExpr {
    core_expr_without_yield(py_expr!("{name:id} is not None", name = name))
}

fn is_resume_generator_exit_test() -> CoreBlockPyExpr {
    core_expr_without_yield(py_expr!("isinstance(_dp_resume_exc, GeneratorExit)"))
}

fn resume_exc_raise_term() -> BlockTerm<CoreBlockPyExpr> {
    BlockTerm::Raise(TermRaise {
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
    kind: FunctionKind,
    name_gen: FunctionNameGen,
    next_resume_pc: usize,
    blocks: Vec<LinearCoreBlock>,
    exception_edges: HashMap<BlockLabel, Option<BlockLabel>>,
    target_arg_indices: HashMap<BlockLabel, Vec<usize>>,
    resume_targets: Vec<(usize, BlockLabel)>,
    exhausted_label: BlockLabel,
}

impl ResumeLoweringState {
    fn new(
        name_gen: FunctionNameGen,
        kind: FunctionKind,
        target_arg_indices: HashMap<BlockLabel, Vec<usize>>,
    ) -> Self {
        let exhausted_label = name_gen.next_block_name();
        Self {
            kind,
            name_gen,
            next_resume_pc: 2,
            blocks: Vec::new(),
            exception_edges: HashMap::new(),
            target_arg_indices,
            resume_targets: Vec::new(),
            exhausted_label,
        }
    }

    fn fresh_label(&mut self, base: &str) -> BlockLabel {
        let _ = base;
        self.name_gen.next_block_name()
    }

    fn fresh_resume_target(&mut self, base: &str) -> (usize, BlockLabel) {
        let pc = self.next_resume_pc;
        self.next_resume_pc += 1;
        let label = self.fresh_label(base);
        self.resume_targets.push((pc, label.clone()));
        (pc, label)
    }

    fn fresh_temp(&mut self, base: &str) -> String {
        self.name_gen.next_tmp_name(base).to_string()
    }

    fn push_block(&mut self, mut block: LinearCoreBlock, exc_target: Option<BlockLabel>) {
        let active_exception = block
            .params
            .iter()
            .find(|param| param.role == BlockParamRole::Exception)
            .map(|param| core_name(param.name.as_str()))
            .unwrap_or_else(core_none);
        block.body.insert(
            0,
            internal_store_stmt("_dp_throw_context", active_exception),
        );
        self.exception_edges.insert(block.label.clone(), exc_target);
        self.blocks.push(block);
    }

    fn prune_term_target_args(&self, term: &mut BlockTerm<CoreBlockPyExpr>) {
        let BlockTerm::Jump(edge) = term else {
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
    label: BlockLabel,
    body: Vec<LinearYieldStmt>,
    term: BlockTerm<CoreBlockPyExprWithYield>,
    params: Vec<BlockParam>,
    exc_target: Option<BlockLabel>,
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
            BlockTerm::Return(CoreBlockPyExprWithYield::implicit_none_expr()),
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
        BlockTerm::Return(value) => {
            push_completion_raise_block(
                state,
                label,
                lowered_body,
                Some(ErrOnYield.try_map_expr(value).unwrap_or_else(|_| {
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
    label: BlockLabel,
    prefix: &mut Vec<LinearCoreStmt>,
    site: YieldSite,
    tail_body: Vec<LinearYieldStmt>,
    tail_term: BlockTerm<CoreBlockPyExprWithYield>,
    params: Vec<BlockParam>,
    exc_target: Option<BlockLabel>,
) {
    match site {
        YieldSite::ExprYield(value) => {
            let (resume_pc, resume_label) = state.fresh_resume_target("yield_resume");
            prefix.push(internal_store_stmt("_dp_pc", core_literal_int(resume_pc)));
            prefix.push(internal_store_stmt("_dp_yieldfrom", core_none()));
            state.push_block(
                BlockPyBlock {
                    label,
                    body: std::mem::take(prefix),
                    term: BlockTerm::Return(yield_value_expr(value)),
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
            prefix.push(internal_store_stmt("_dp_pc", core_literal_int(resume_pc)));
            prefix.push(internal_store_stmt("_dp_yieldfrom", core_none()));
            state.push_block(
                BlockPyBlock {
                    label,
                    body: std::mem::take(prefix),
                    term: BlockTerm::Return(yield_value_expr(value)),
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
            prefix.push(internal_store_stmt("_dp_pc", core_literal_int(resume_pc)));
            prefix.push(internal_store_stmt("_dp_yieldfrom", core_none()));
            state.push_block(
                BlockPyBlock {
                    label,
                    body: std::mem::take(prefix),
                    term: BlockTerm::Return(yield_value_expr(value)),
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
                BlockTerm::Return(unresolved_load_expr(expr_name("_dp_send_value"))),
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
            BlockTerm::Return(unresolved_load_expr(expr_name("_dp_yield_from_value"))),
            params,
            exc_target,
        ),
    }
}

fn emit_resume_after_yield(
    state: &mut ResumeLoweringState,
    resume_label: BlockLabel,
    assign_target: Option<UnresolvedName>,
    mut tail_body: Vec<LinearYieldStmt>,
    tail_term: BlockTerm<CoreBlockPyExprWithYield>,
    params: Vec<BlockParam>,
    exc_target: Option<BlockLabel>,
) {
    let raise_label = state.fresh_label("yield_throw");
    let continue_label = state.fresh_label("yield_continue");
    state.push_block(
        BlockPyBlock {
            label: resume_label,
            body: Vec::new(),
            term: BlockTerm::IfTerm(TermIf {
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
            unresolved_store_stmt(target, unresolved_load_expr(expr_name("_dp_send_value"))),
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
    label: BlockLabel,
    prefix: &mut Vec<LinearCoreStmt>,
    value: CoreBlockPyExprWithYield,
    assign_target: Option<UnresolvedName>,
    mut tail_body: Vec<LinearYieldStmt>,
    tail_term: BlockTerm<CoreBlockPyExprWithYield>,
    params: Vec<BlockParam>,
    exc_target: Option<BlockLabel>,
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
    let value_expr = ErrOnYield
        .try_map_expr(value)
        .unwrap_or_else(|_| panic!("yield from payload unexpectedly contained nested yield"));
    let yielded_value_name = state.fresh_temp("yield_from_value");
    let throw_name = state.fresh_temp("yield_from_throw");
    let close_name = state.fresh_temp("yield_from_close");
    let caught_exc_name = state.fresh_temp("yield_from_exc");
    prefix.push(internal_store_stmt(
        "_dp_yieldfrom",
        core_call("iter", vec![value_expr]),
    ));
    prefix.push(internal_store_stmt("_dp_pc", core_literal_int(delegate_pc)));
    state.push_block(
        BlockPyBlock {
            label,
            body: std::mem::take(prefix),
            term: BlockTerm::Jump(BlockEdge::new(delegate_label.clone())),
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
            term: BlockTerm::IfTerm(TermIf {
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
            term: BlockTerm::IfTerm(TermIf {
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
            body: vec![internal_store_stmt(
                yielded_value_name.as_str(),
                core_expr_without_yield(py_expr!("next(_dp_yieldfrom)")),
            )],
            term: BlockTerm::Jump(BlockEdge::new(yielded_label.clone())),
            params: params.clone(),
            exc_edge: None,
        },
        Some(call_except_label.clone()),
    );
    state.push_block(
        BlockPyBlock {
            label: send_call_label,
            body: vec![internal_store_stmt(
                yielded_value_name.as_str(),
                core_expr_without_yield(py_expr!("_dp_yieldfrom.send(_dp_send_value)")),
            )],
            term: BlockTerm::Jump(BlockEdge::new(yielded_label.clone())),
            params: params.clone(),
            exc_edge: None,
        },
        Some(call_except_label.clone()),
    );
    state.push_block(
        BlockPyBlock {
            label: exc_dispatch_label,
            body: Vec::new(),
            term: BlockTerm::IfTerm(TermIf {
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
            body: vec![internal_store_stmt(
                close_name.as_str(),
                core_expr_without_yield(py_expr!("getattr(_dp_yieldfrom, \"close\", None)")),
            )],
            term: BlockTerm::IfTerm(TermIf {
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
            body: vec![core_expr_without_yield(py_expr!(
                "{close:id}()",
                close = close_name.as_str(),
            ))],
            term: BlockTerm::Jump(BlockEdge::new(raise_resume_exc_label.clone())),
            params: params.clone(),
            exc_edge: None,
        },
        exc_target.clone(),
    );
    state.push_block(
        BlockPyBlock {
            label: throw_lookup_label,
            body: vec![internal_store_stmt(
                throw_name.as_str(),
                core_expr_without_yield(py_expr!("getattr(_dp_yieldfrom, \"throw\", None)")),
            )],
            term: BlockTerm::IfTerm(TermIf {
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
            body: vec![internal_store_stmt(
                yielded_value_name.as_str(),
                core_expr_without_yield(py_expr!(
                    "{throw_fn:id}(_dp_resume_exc)",
                    throw_fn = throw_name.as_str(),
                )),
            )],
            term: BlockTerm::Jump(BlockEdge::new(yielded_label.clone())),
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
            term: BlockTerm::IfTerm(TermIf {
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
            body: vec![internal_store_stmt(
                yielded_value_name.as_str(),
                current_exception_value_expr(caught_exc_name.as_str()),
            )],
            term: BlockTerm::Jump(BlockEdge::with_args(
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
            term: BlockTerm::Raise(TermRaise {
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
            body: vec![internal_store_stmt("_dp_pc", core_literal_int(delegate_pc))],
            term: BlockTerm::Return(core_name(yielded_value_name.as_str())),
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
        internal_store_stmt(
            "_dp_yieldfrom",
            CoreBlockPyExprWithYield::implicit_none_expr(),
        ),
    );
    if let Some(target) = assign_target {
        tail_body.insert(
            1,
            unresolved_store_stmt(
                target,
                unresolved_load_expr(expr_name(yielded_value_name.as_str())),
            ),
        );
    } else if matches!(tail_term, BlockTerm::Return(CoreBlockPyExprWithYield::Load(ref op)) if op.name.id_str() == "_dp_yield_from_value")
    {
        tail_body.insert(
            1,
            internal_store_stmt(
                "_dp_yield_from_value",
                unresolved_load_expr(expr_name(yielded_value_name.as_str())),
            ),
        );
    }
    lower_resume_fragment(state, done_label, tail_body, tail_term, params, exc_target);
}

fn lower_resume_blocks(
    callable: &BlockPyFunction<CoreBlockPyPassWithYield>,
    resume_name_gen: FunctionNameGen,
) -> (
    Vec<LinearCoreBlock>,
    HashMap<BlockLabel, Option<BlockLabel>>,
    BlockLabel,
) {
    let relabel = callable
        .blocks
        .iter()
        .map(|block| (block.label, resume_name_gen.next_block_name()))
        .collect::<HashMap<_, _>>();
    let linear_exception_edges = lowered_exception_edges(&callable.blocks);
    let declared_param_indices_by_label = callable
        .blocks
        .iter()
        .map(|block| {
            (
                relabel
                    .get(&block.label)
                    .expect("resume relabel should cover every block")
                    .clone(),
                generator_resume_declared_param_indices(callable.kind, &block.params),
            )
        })
        .collect::<HashMap<_, _>>();
    let linear_blocks = callable
        .blocks
        .iter()
        .cloned()
        .map(|block| {
            let mut term = block.term;
            term.relabel_targets(&relabel);
            LinearYieldBlock {
                label: relabel
                    .get(&block.label)
                    .expect("resume relabel should cover every block")
                    .clone(),
                body: block.body,
                term,
                params: block.params,
                exc_edge: None,
            }
        })
        .collect::<Vec<_>>();
    let remapped_exception_edges = linear_exception_edges
        .into_iter()
        .map(|(label, exc_target)| {
            (
                relabel
                    .get(&label)
                    .expect("resume relabel should cover every exception source")
                    .clone(),
                exc_target.map(|target| {
                    relabel
                        .get(&target)
                        .expect("resume relabel should cover every exception target")
                        .clone()
                }),
            )
        })
        .collect::<HashMap<_, _>>();
    let resume_entry_target = relabel
        .get(&callable.entry_block().label)
        .expect("resume relabel should cover entry block")
        .clone();

    let mut state = ResumeLoweringState::new(
        resume_name_gen,
        callable.kind,
        declared_param_indices_by_label,
    );
    state.resume_targets.push((1, resume_entry_target));

    let mut queue = linear_blocks
        .into_iter()
        .map(|block| {
            (
                block.label.clone(),
                block.body,
                block.term,
                generator_resume_declared_params(callable.kind, &block.params),
                remapped_exception_edges
                    .get(&block.label)
                    .cloned()
                    .unwrap_or(None),
            )
        })
        .collect::<VecDeque<_>>();
    while let Some((label, body, term, params, exc_target)) = queue.pop_front() {
        lower_resume_fragment(&mut state, label, body, term, params, exc_target);
    }

    let dispatch_label = state.fresh_label("resume_dispatch");
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
        term: BlockTerm::BranchTable(TermBranchTable {
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

fn ordered_resume_binding_logical_names(
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
    let resume_binding_logical_names =
        ordered_resume_binding_logical_names(&callable, &persistent_state_order);
    let (resume_blocks, _resume_exception_edges, _resume_entry_label) =
        lower_resume_blocks(&callable, resume_name_gen.share());
    let closure_bindings =
        resume_closure_bindings(&callable.semantic, &resume_binding_logical_names);

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
        kind: FunctionKind::Function,
        params: resume_params.clone(),
        blocks: resume_blocks.clone(),
        doc: None,
        storage_layout: None,
        semantic: resume_semantic,
    };
    let resume_storage_layout = compute_storage_layout_from_semantics(&resume_function)
        .unwrap_or_else(|| panic!("generator resume should compute a storage layout"));
    let resume_function = BlockPyFunction {
        storage_layout: Some(resume_storage_layout),
        ..resume_function
    };

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

    vec![visible_function, resume_function]
}

#[cfg(test)]
mod test;
