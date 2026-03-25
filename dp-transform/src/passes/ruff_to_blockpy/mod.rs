use crate::block_py::cfg::{
    fold_constant_brif_blockpy, fold_jumps_to_trivial_none_return_blockpy,
    prune_unreachable_blockpy_blocks,
};
use crate::block_py::dataflow::{
    analyze_blockpy_use_def, compute_block_params_blockpy,
    extend_state_order_with_declared_block_params, loaded_names_in_blockpy_block,
    merge_declared_block_params,
};
use crate::block_py::exception::{
    contains_return_stmt_in_body, contains_return_stmt_in_handlers,
    rewrite_region_returns_to_finally_blockpy,
};
use crate::block_py::param_specs::ParamSpec;
use crate::block_py::state::collect_state_vars;
use crate::block_py::{
    assert_blockpy_block_normalized, move_entry_block_to_front, BlockPyBindingKind,
    BlockPyCallableFacts, BlockPyCallableSemanticInfo, BlockPyEdge, BlockPyFallthroughTerm,
    BlockPyFunction, BlockPyFunctionKind, BlockPyLabel, BlockPyPass, BlockPyStmt, BlockPyTerm,
    CfgBlock, ClosureLayout, FunctionName, FunctionNameGen,
};
use crate::namegen::fresh_name;
use crate::passes::ast_to_ast::context::Context;
use crate::passes::ast_to_ast::expr_utils::make_tuple;
use crate::passes::ast_to_ast::scope_helpers::cell_name;
use crate::passes::RuffBlockPyPass;
use crate::ruff_ast_to_string;
use crate::template::is_simple;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt};
use std::collections::{HashMap, HashSet};
mod compat;
mod deleted_name_loads;
pub(crate) mod expr_lowering;
mod module_plan;
mod stmt_lowering;
mod stmt_sequences;
mod try_regions;
pub(crate) use deleted_name_loads::rewrite_deleted_name_loads;

pub(crate) use super::blockpy_generators::build_blockpy_closure_layout;
pub(crate) use module_plan::rewrite_ast_to_lowered_blockpy_module_plan_with_module;

pub(crate) use compat::{
    compat_block_from_blockpy, compat_block_from_blockpy_with_exc_target, emit_for_loop_blocks,
    emit_if_branch_block_with_expr_setup, emit_sequence_jump_block,
    emit_sequence_raise_block_with_expr_setup, emit_sequence_return_block_with_expr_setup,
    emit_simple_while_blocks_with_expr_setup,
};
#[cfg(test)]
use stmt_lowering::lower_stmt_into;
pub(crate) use stmt_lowering::{
    build_for_target_assign_body, lower_star_try_stmt_sequence, lower_try_stmt_sequence,
    lower_with_stmt_sequence, rewrite_assign_stmt, rewrite_delete_stmt,
};
pub(crate) use stmt_sequences::{
    lower_expanded_stmt_sequence, lower_stmt_sequence_with_state, lower_stmts_to_blockpy_stmts,
};
pub(crate) use try_regions::{
    block_references_label, build_try_plan, finalize_try_regions, lower_try_regions,
    prepare_except_body, prepare_finally_body, TryPlan,
};

pub(crate) type LoweredBlockPyBlock<E = Expr> = CfgBlock<BlockPyStmt<E>, BlockPyTerm<E>>;
pub(crate) type BlockPyBlock<E = Expr> = LoweredBlockPyBlock<E>;

#[derive(Clone)]
pub(crate) enum StmtSequenceHeadPlan {
    Linear(Stmt),
    Expanded(Vec<Stmt>),
    FunctionDef(ast::StmtFunctionDef),
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

pub(crate) fn attach_exception_edges_to_blocks<E>(
    blocks: Vec<crate::block_py::BlockPyBlock<E>>,
    exception_edges: &HashMap<String, Option<String>>,
) -> Vec<LoweredBlockPyBlock<E>> {
    blocks
        .into_iter()
        .map(|block| CfgBlock {
            label: block.label.clone(),
            body: block.body,
            term: block.term,
            params: block.params,
            exc_edge: exception_edges
                .get(block.label.as_str())
                .cloned()
                .flatten()
                .map(BlockPyLabel::from)
                .map(BlockPyEdge::new),
        })
        .collect()
}

pub(crate) fn lowered_exception_edges<S, T>(
    blocks: &[CfgBlock<S, T>],
) -> HashMap<String, Option<String>> {
    blocks
        .iter()
        .map(|block| {
            (
                block.label.as_str().to_string(),
                block.exc_edge.as_ref().map(|edge| edge.target.to_string()),
            )
        })
        .collect()
}

fn append_closure_storage_aliases(
    block_params: &mut HashMap<String, Vec<String>>,
    layout: &ClosureLayout,
) {
    let logical_name_by_storage = layout
        .cellvars
        .iter()
        .chain(layout.runtime_cells.iter())
        .filter(|slot| slot.logical_name != slot.storage_name)
        .map(|slot| (slot.storage_name.as_str(), slot.logical_name.as_str()))
        .collect::<HashMap<_, _>>();
    for params in block_params.values_mut() {
        let mut logical_aliases = Vec::new();
        for param_name in params.iter() {
            let Some(logical_name) = logical_name_by_storage.get(param_name.as_str()).copied()
            else {
                continue;
            };
            if params.iter().any(|existing| existing == logical_name)
                || logical_aliases
                    .iter()
                    .any(|existing| existing == logical_name)
            {
                continue;
            }
            logical_aliases.push(logical_name.to_string());
        }
        params.extend(logical_aliases);
    }
}

pub(crate) fn should_include_closure_storage_aliases<P>(function: &BlockPyFunction<P>) -> bool
where
    P: BlockPyPass,
{
    matches!(
        function.kind,
        BlockPyFunctionKind::Coroutine
            | BlockPyFunctionKind::Generator
            | BlockPyFunctionKind::AsyncGenerator
    ) || function.names.fn_name == "_dp_resume"
}

pub(crate) fn recompute_lowered_block_params<P>(
    function: &BlockPyFunction<P>,
    include_closure_storage_aliases: bool,
) -> HashMap<String, Vec<String>>
where
    P: BlockPyPass,
{
    let param_names = function.params.names();
    let mut state_vars = collect_state_vars(&param_names, &function.blocks);
    for block in &function.blocks {
        let Some(exc_param) = block.exception_param() else {
            continue;
        };
        if !state_vars.iter().any(|existing| existing == exc_param) {
            state_vars.push(exc_param.to_string());
        }
    }
    extend_state_order_with_declared_block_params(&function.blocks, &mut state_vars);

    let mut extra_successors = HashMap::new();
    for (source, target) in lowered_exception_edges(&function.blocks) {
        let Some(target) = target else {
            continue;
        };
        extra_successors
            .entry(source)
            .or_insert_with(Vec::new)
            .push(target);
    }
    let mut block_params =
        compute_block_params_blockpy(&function.blocks, &state_vars, &extra_successors);
    merge_declared_block_params(&function.blocks, &mut block_params);
    if include_closure_storage_aliases {
        if let Some(layout) = function.closure_layout.as_ref() {
            append_closure_storage_aliases(&mut block_params, layout);
        }
    }
    block_params
}

fn build_semantic_blockpy_closure_layout(
    callable_def: &BlockPyFunction<RuffBlockPyPass>,
    injected_exception_names: &HashSet<String>,
) -> Option<ClosureLayout> {
    let param_names = callable_def.params.names();
    let local_cell_slot_names = callable_def.semantic.local_cell_storage_names();
    let mut local_cell_slots = local_cell_slot_names.iter().cloned().collect::<Vec<_>>();
    local_cell_slots.sort();
    let param_name_set = param_names.iter().cloned().collect::<HashSet<_>>();
    let used_names: HashSet<String> = callable_def
        .blocks
        .iter()
        .flat_map(|block| loaded_names_in_blockpy_block(block).into_iter())
        .collect();
    let defined_names: HashSet<String> = callable_def
        .blocks
        .iter()
        .flat_map(|block| analyze_blockpy_use_def(block).1.into_iter())
        .collect();
    let mut referenced_names = used_names
        .iter()
        .chain(defined_names.iter())
        .cloned()
        .collect::<Vec<_>>();
    referenced_names.sort();
    referenced_names.dedup();
    let mut capture_names = referenced_names
        .iter()
        .filter(|name| !param_name_set.contains(name.as_str()))
        .filter(|name| {
            if *name == "_dp_classcell" {
                if param_name_set.contains("_dp_classcell_arg")
                    || defined_names.contains(name.as_str())
                {
                    return false;
                }
                return true;
            }
            if name.starts_with("_dp_cell_") {
                return !local_cell_slot_names.contains(name.as_str())
                    && !defined_names.contains(name.as_str());
            }
            if callable_def
                .semantic
                .resolved_load_binding_kind(name.as_str())
                == BlockPyBindingKind::Cell(crate::block_py::BlockPyCellBindingKind::Capture)
            {
                let capture_name = cell_name(name.as_str());
                return !local_cell_slot_names.contains(capture_name.as_str())
                    && !defined_names.contains(capture_name.as_str());
            }
            false
        })
        .cloned()
        .collect::<Vec<_>>();
    capture_names.sort();
    capture_names.dedup();
    if capture_names.is_empty()
        && local_cell_slots.is_empty()
        && injected_exception_names.is_empty()
    {
        return None;
    }

    let mut state_vars = collect_state_vars(&param_names, &callable_def.blocks);
    for capture_name in &capture_names {
        if !state_vars.iter().any(|existing| existing == capture_name) {
            state_vars.push(capture_name.clone());
        }
    }
    for slot in local_cell_slots {
        let logical_name = slot.strip_prefix("_dp_cell_").unwrap_or(&slot).to_string();
        if !state_vars.iter().any(|existing| existing == &logical_name) {
            state_vars.push(logical_name);
        }
    }

    Some(build_blockpy_closure_layout(
        &param_names,
        &state_vars,
        &capture_names,
        injected_exception_names,
    ))
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_blockpy_callable_def_from_runtime_input(
    context: &Context,
    name_gen: FunctionNameGen,
    names: FunctionName,
    params: ParamSpec,
    runtime_input_body: &[Stmt],
    doc: Option<String>,
    end_label: BlockPyLabel,
    blockpy_kind: BlockPyFunctionKind,
    facts: &BlockPyCallableFacts,
    semantic: &BlockPyCallableSemanticInfo,
) -> BlockPyFunction<RuffBlockPyPass> {
    let function_id = name_gen.function_id();
    let mut blocks = Vec::new();
    let entry_label = lower_stmt_sequence_with_state(
        context,
        runtime_input_body,
        RegionTargets::new(end_label.to_string(), None),
        &mut blocks,
        &name_gen,
    );
    move_entry_block_to_front(&mut blocks, entry_label.as_str());
    for block in &blocks {
        assert_blockpy_block_normalized(block);
    }
    let mut callable_def = BlockPyFunction {
        function_id,
        name_gen,
        names,
        kind: blockpy_kind,
        params,
        blocks,
        doc,
        closure_layout: None,
        facts: facts.clone(),
        semantic: semantic.clone(),
    };
    let needs_end_block = entry_label == end_label.as_str()
        || callable_def
            .blocks
            .iter()
            .any(|block| block_references_label(block, end_label.as_str()));
    if needs_end_block {
        callable_def.blocks.push(CfgBlock {
            label: end_label,
            body: Vec::new(),
            term: BlockPyTerm::implicit_function_return(),
            params: Vec::new(),
            exc_edge: None,
        });
    }
    fold_jumps_to_trivial_none_return_blockpy(&mut callable_def.blocks);
    fold_constant_brif_blockpy(&mut callable_def.blocks);
    let extra_roots = callable_def
        .blocks
        .iter()
        .filter_map(|block| block.exc_edge.as_ref().map(|edge| edge.target.to_string()))
        .collect::<Vec<_>>();
    prune_unreachable_blockpy_blocks(entry_label.as_str(), &extra_roots, &mut callable_def.blocks);
    callable_def.closure_layout =
        build_semantic_blockpy_closure_layout(&callable_def, &HashSet::new());
    callable_def
}

pub(crate) fn recompute_semantic_blockpy_closure_layout(
    callable_def: &BlockPyFunction<RuffBlockPyPass>,
) -> Option<ClosureLayout> {
    build_semantic_blockpy_closure_layout(callable_def, &HashSet::new())
}

#[derive(Clone)]
pub(crate) struct LoopContext {
    continue_label: BlockPyLabel,
    break_label: BlockPyLabel,
}

#[derive(Clone)]
pub(crate) struct LoopLabels {
    pub break_label: String,
    pub continue_label: String,
}

#[derive(Clone)]
pub(crate) struct RegionTargets {
    pub normal_cont: String,
    pub loop_labels: Option<LoopLabels>,
    pub active_exc: Option<String>,
}

impl RegionTargets {
    pub(crate) fn new(normal_cont: String, active_exc: Option<String>) -> Self {
        Self {
            normal_cont,
            loop_labels: None,
            active_exc,
        }
    }

    pub(crate) fn nested(&self, normal_cont: String) -> Self {
        Self {
            normal_cont,
            loop_labels: self.loop_labels.clone(),
            active_exc: self.active_exc.clone(),
        }
    }

    pub(crate) fn nested_with_loop(
        &self,
        normal_cont: String,
        loop_labels: Option<LoopLabels>,
    ) -> Self {
        Self {
            normal_cont,
            loop_labels,
            active_exc: self.active_exc.clone(),
        }
    }
}

fn assign_delete_error(message: &str, stmt: &Stmt) -> String {
    format!("{message}\nstmt:\n{}", ruff_ast_to_string(stmt).trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block_py::{
        BlockPyEdge, BlockPyFunction, BlockPyLabel, BlockPyModule, BlockPyPass, BlockPyRaise,
        BlockPyStmt, BlockPyTerm, CoreBlockPyExpr,
    };
    use crate::passes::ast_to_ast::{context::Context, Options};
    use crate::passes::ruff_to_blockpy::stmt_sequences::{
        lower_for_stmt_sequence, lower_if_stmt_sequence, lower_if_stmt_sequence_from_stmt,
        lower_while_stmt_sequence, lower_while_stmt_sequence_from_stmt, plan_stmt_sequence_head,
    };
    use crate::passes::ruff_to_blockpy::try_regions::build_try_plan;
    use crate::passes::{CoreBlockPyPass, RuffBlockPyPass};
    use crate::{transform_str_to_blockpy_with_options, transform_str_to_ruff_with_options};

    fn test_name_gen() -> FunctionNameGen {
        let mut module_name_gen = crate::block_py::ModuleNameGen::new(0);
        module_name_gen.next_function_name_gen()
    }

    fn wrapped_blockpy(source: &str) -> BlockPyModule<RuffBlockPyPass> {
        transform_str_to_blockpy_with_options(source, Options::for_test()).unwrap()
    }

    fn wrapped_semantic_blockpy(source: &str) -> BlockPyModule<RuffBlockPyPass> {
        transform_str_to_ruff_with_options(source, Options::for_test())
            .unwrap()
            .get_pass::<BlockPyModule<RuffBlockPyPass>>("semantic_blockpy")
            .cloned()
            .expect("semantic_blockpy pass should be tracked")
    }

    fn wrapped_core_blockpy(source: &str) -> BlockPyModule<CoreBlockPyPass> {
        transform_str_to_ruff_with_options(source, Options::for_test())
            .unwrap()
            .get_pass::<BlockPyModule<CoreBlockPyPass>>("core_blockpy")
            .cloned()
            .expect("core_blockpy pass should be tracked")
    }

    fn function_by_name<'a, P: BlockPyPass>(
        blockpy: &'a BlockPyModule<P>,
        bind_name: &str,
    ) -> &'a BlockPyFunction<P> {
        blockpy
            .callable_defs
            .iter()
            .find(|func| func.names.bind_name == bind_name)
            .unwrap_or_else(|| panic!("missing BlockPy function {bind_name}; got {blockpy:?}"))
    }

    fn lower_stmt_for_panic_test(stmt: &Stmt) {
        let context = Context::new(Options::for_test(), "");
        let mut out = crate::block_py::BlockPyCfgFragmentBuilder::<BlockPyStmt, BlockPyTerm>::new();
        let mut next_label_id = 0usize;
        let _ = lower_stmt_into(&context, stmt, &mut out, None, &mut next_label_id);
    }

    fn test_context() -> Context {
        Context::new(Options::for_test(), "")
    }

    #[test]
    fn lowers_post_simplification_control_flow() {
        let blockpy = wrapped_blockpy(
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
        );
        let blocks = &function_by_name(&blockpy, "f").blocks;
        let rendered = crate::block_py::pretty::blockpy_module_to_string(&blockpy);
        assert!(blocks
            .iter()
            .any(|block| matches!(block.term, BlockPyTerm::IfTerm(_))));
        assert!(
            blocks.iter().any(|block| block.exc_edge.is_some()),
            "{rendered}"
        );
        assert!(rendered.contains("return x"), "{rendered}");
    }

    #[test]
    fn lowers_async_for_structurally() {
        let blockpy = wrapped_semantic_blockpy(
            r#"
async def f(xs):
    async for x in xs:
        body(x)
"#,
        );
        let rendered = crate::block_py::pretty::blockpy_module_to_string(&blockpy);
        assert!(
            rendered.contains("await __dp_anext_or_sentinel"),
            "{rendered}"
        );
        assert!(rendered.contains("__dp_anext_or_sentinel"), "{rendered}");
    }

    #[test]
    fn lowers_generator_yield_to_explicit_blockpy_dispatch() {
        let blockpy = wrapped_core_blockpy(
            r#"
def gen(n):
    yield n
"#,
        );
        let rendered = crate::block_py::pretty::blockpy_module_to_string(&blockpy);
        assert!(rendered.contains("generator gen(n):"), "{rendered}");
        assert!(
            rendered.contains("function gen(_dp_self, _dp_send_value, _dp_resume_exc):"),
            "{rendered}"
        );
        assert!(
            rendered.contains("return __dp_make_closure_generator"),
            "{rendered}"
        );
        assert!(rendered.contains("branch_table"), "{rendered}");
        assert!(!rendered.contains("yield n"), "{rendered}");
    }

    #[test]
    fn stmt_sequence_head_plan_leaves_yield_expr_linear() {
        let module = ruff_python_parser::parse_module(
            r#"
def gen():
    yield x
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = &module[0] else {
            panic!("expected function def");
        };
        let stmt = &func.body[0];

        assert!(matches!(
            plan_stmt_sequence_head(&test_context(), stmt),
            StmtSequenceHeadPlan::Linear(_)
        ));
    }

    #[test]
    fn stmt_sequence_head_plan_leaves_assign_yield_linear() {
        let module = ruff_python_parser::parse_module(
            r#"
def gen():
    x = (yield y)
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = &module[0] else {
            panic!("expected function def");
        };
        let stmt = &func.body[0];

        assert!(matches!(
            plan_stmt_sequence_head(&test_context(), stmt),
            StmtSequenceHeadPlan::Linear(_)
        ));
    }

    #[test]
    fn stmt_sequence_head_plan_keeps_plain_return_as_plain_return() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    return x
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = &module[0] else {
            panic!("expected function def");
        };
        let stmt = &func.body[0];

        assert!(matches!(
            plan_stmt_sequence_head(&test_context(), stmt),
            StmtSequenceHeadPlan::Return(_)
        ));
    }

    #[test]
    fn stmt_sequence_head_plan_keeps_return_yield_as_plain_return() {
        let module = ruff_python_parser::parse_module(
            r#"
def gen(n):
    return (yield n)
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = &module[0] else {
            panic!("expected function def");
        };
        let stmt = &func.body[0];

        assert!(matches!(
            plan_stmt_sequence_head(&test_context(), stmt),
            StmtSequenceHeadPlan::Return(_)
        ));
    }

    #[test]
    fn stmt_sequence_head_plan_simplifies_assert_to_if() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    assert cond, msg
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = &module[0] else {
            panic!("expected function def");
        };
        let stmt = &func.body[0];

        assert!(matches!(
            plan_stmt_sequence_head(&test_context(), stmt),
            StmtSequenceHeadPlan::If(_)
        ));
    }

    #[test]
    fn stmt_sequence_head_plan_simplifies_match_to_expanded_if_chain() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    match "aa":
        case str(slot):
            return slot
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = &module[0] else {
            panic!("expected function def");
        };
        let stmt = &func.body[0];

        let StmtSequenceHeadPlan::Expanded(body) = plan_stmt_sequence_head(&test_context(), stmt)
        else {
            panic!("expected expanded match body");
        };
        assert!(matches!(body[0], Stmt::Assign(_)));
        assert!(body.iter().any(|stmt| matches!(stmt, Stmt::If(_))));
    }

    #[test]
    fn stmt_sequence_head_plan_re_expands_builtin_match_if_head() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    match "aa":
        case str(slot):
            return slot
        case _:
            return None
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = &module[0] else {
            panic!("expected function def");
        };
        let stmt = &func.body[0];

        let StmtSequenceHeadPlan::Expanded(body) = plan_stmt_sequence_head(&test_context(), stmt)
        else {
            panic!("expected expanded match body");
        };
        let match_if = body
            .iter()
            .find(|stmt| matches!(stmt, Stmt::If(_)))
            .expect("expected expanded match body to contain an if");

        assert!(
            matches!(
                plan_stmt_sequence_head(&test_context(), match_if),
                StmtSequenceHeadPlan::If(_)
            ),
            "{}",
            crate::ruff_ast_to_string(match_if).trim_end()
        );
    }

    #[test]
    fn blockpy_match_builtin_class_pattern_lowers_short_circuit_test_before_bb() {
        let blockpy = wrapped_blockpy(
            r#"
def f():
    match "aa":
        case str(slot):
            return slot
        case _:
            return None
"#,
        );
        let rendered = crate::block_py::pretty::blockpy_module_to_string(&blockpy);
        assert!(
            !rendered.contains("and __dp_match_class_attr_exists"),
            "{rendered}"
        );
        assert!(
            !rendered.contains("and __dp_match_class_attr_value"),
            "{rendered}"
        );
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
        let ast::Stmt::FunctionDef(func) = &module[0] else {
            panic!("expected function def");
        };
        let ast::Stmt::For(for_stmt) = &func.body[0] else {
            panic!("expected for stmt");
        };

        let mut blocks = Vec::new();
        let entry = lower_for_stmt_sequence(
            for_stmt.clone(),
            &[],
            RegionTargets::new("cont".to_string(), None),
            Vec::new(),
            &mut blocks,
            "_dp_iter_0",
            "_dp_tmp_0",
            BlockPyLabel::from("_dp_bb_demo_0"),
            BlockPyLabel::from("_dp_bb_demo_0"),
            BlockPyLabel::from("_dp_bb_demo_1"),
            BlockPyLabel::from("_dp_bb_demo_2"),
            vec![py_stmt!("x = _dp_tmp_0"), py_stmt!("_dp_tmp_0 = None")],
            &mut |_stmts: &[Stmt], targets: RegionTargets, _blocks: &mut Vec<BlockPyBlock>| {
                targets.normal_cont
            },
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
    fn lower_with_stmt_sequence_expands_via_structured_desugar() {
        let module = ruff_python_parser::parse_module(
            r#"
def f(ctx, value):
    with ctx() as value:
        body()
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = &module[0] else {
            panic!("expected function def");
        };
        let ast::Stmt::With(with_stmt) = &func.body[0] else {
            panic!("expected with stmt");
        };

        let mut blocks = Vec::new();
        let name_gen = test_name_gen();
        let mut saw_try_stmt = false;
        let mut saw_with_ok_assign = false;
        let entry = lower_with_stmt_sequence(
            with_stmt.clone(),
            &[],
            RegionTargets::new("cont".to_string(), None),
            Vec::new(),
            &mut blocks,
            &name_gen,
            false,
            &mut |_expanded: &[Stmt], targets: RegionTargets, _blocks: &mut Vec<BlockPyBlock>| {
                saw_try_stmt = _expanded
                    .iter()
                    .any(|stmt| matches!(stmt, ast::Stmt::Try(_)));
                saw_with_ok_assign = _expanded.iter().any(|stmt| {
                    match stmt {
                    ast::Stmt::Assign(assign) => assign.targets.iter().any(|target| {
                        matches!(target, Expr::Name(name) if name.id.as_str().contains("with_ok"))
                    }),
                    _ => false,
                }
                });
                targets.normal_cont
            },
        );

        assert_eq!(entry, "cont");
        assert!(blocks.is_empty());
        assert!(saw_try_stmt);
        assert!(saw_with_ok_assign);
    }

    #[test]
    fn lower_try_stmt_sequence_emits_entry_jump_and_except_edge() {
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
        let ast::Stmt::FunctionDef(func) = &module[0] else {
            panic!("expected function def");
        };
        let ast::Stmt::Try(try_stmt) = &func.body[0] else {
            panic!("expected try stmt");
        };

        let mut blocks = Vec::new();
        let name_gen = test_name_gen();
        let try_plan = build_try_plan(&name_gen, false, false);
        let entry = lower_try_stmt_sequence(
            try_stmt.clone(),
            &[],
            RegionTargets::new("cont".to_string(), None),
            Vec::new(),
            &mut blocks,
            BlockPyLabel::from("_dp_bb_demo_legacy"),
            try_plan,
            &mut |_expanded: &[Stmt], targets: RegionTargets, blocks: &mut Vec<BlockPyBlock>| {
                let label = format!("lowered_{}", blocks.len());
                blocks.push(
                    crate::passes::ruff_to_blockpy::compat::compat_block_from_blockpy_with_exc_target(
                        BlockPyLabel::from(label.clone()),
                        Vec::new(),
                        BlockPyTerm::Jump(BlockPyEdge::new(BlockPyLabel::from(
                            targets.normal_cont,
                        ))),
                        targets.active_exc.as_deref(),
                    ),
                );
                label
            },
        );

        assert!(!entry.is_empty());
        let Some(try_entry_block) = blocks.iter().find(|block| block.label.as_str() == entry)
        else {
            panic!("expected try entry block");
        };
        let BlockPyTerm::Jump(try_body_edge) = &try_entry_block.term else {
            panic!("expected try entry jump");
        };
        let Some(body_block) = blocks
            .iter()
            .find(|block| block.label.as_str() == try_body_edge.as_str())
        else {
            panic!("expected try body block");
        };
        let exc_edge = body_block
            .exc_edge
            .as_ref()
            .expect("try body block must carry except edge");
        assert_ne!(exc_edge.target.as_str(), try_body_edge.as_str());
        assert!(
            blocks
                .iter()
                .any(|block| block.label.as_str() == exc_edge.target.as_str()),
            "except edge target should resolve to another block"
        );
    }

    #[test]
    fn expanded_stmt_helper_returns_expanded_entry_without_linear_prefix() {
        let mut blocks = Vec::new();
        let mut saw_expanded = false;
        let entry = lower_expanded_stmt_sequence(
            vec![py_stmt!("pass")],
            &[],
            RegionTargets::new("cont".to_string(), None),
            Vec::new(),
            &mut blocks,
            None,
            &mut |expanded: &[Stmt], targets: RegionTargets, _blocks: &mut Vec<BlockPyBlock>| {
                assert_eq!(expanded.len(), 1);
                assert_eq!(targets.normal_cont, "cont");
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
            vec![py_stmt!("pass")],
            &[],
            RegionTargets::new("cont".to_string(), None),
            vec![py_stmt!("x = 1")],
            &mut blocks,
            Some(BlockPyLabel::from("prefix")),
            &mut |_expanded: &[Stmt], _targets: RegionTargets, _blocks: &mut Vec<BlockPyBlock>| {
                "expanded_entry".to_string()
            },
        );

        assert_eq!(entry, "prefix");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].label.as_str(), "prefix");
        assert!(matches!(
            &blocks[0].term,
            BlockPyTerm::Jump(label) if label.as_str() == "expanded_entry"
        ));
    }

    #[test]
    fn if_stmt_helper_lowers_both_branches_via_callback() {
        let mut blocks = Vec::new();
        let then_body = vec![py_stmt!("x = 1")];
        let else_body = vec![py_stmt!("x = 2")];
        let mut calls = Vec::new();
        let context = Context::new(crate::passes::ast_to_ast::Options::for_test(), "");

        let entry = lower_if_stmt_sequence(
            &context,
            &mut blocks,
            BlockPyLabel::from("if_label"),
            vec![py_stmt!("prefix = 0")],
            py_expr!("flag"),
            &then_body,
            &else_body,
            "rest".to_string(),
            &RegionTargets::new("rest".to_string(), None),
            &mut |stmts: &[Stmt], targets: RegionTargets, _blocks: &mut Vec<BlockPyBlock>| {
                calls.push((stmts.len(), targets.normal_cont.clone()));
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
        assert!(matches!(blocks[0].term, BlockPyTerm::IfTerm(_)));
    }

    #[test]
    fn sequence_jump_helper_emits_jump_block() {
        let mut blocks = Vec::new();
        let entry = emit_sequence_jump_block(
            &mut blocks,
            BlockPyLabel::from("jump_label"),
            vec![py_stmt!("prefix = 0")],
            "target".to_string(),
            None,
        );

        assert_eq!(entry, "jump_label");
        assert_eq!(blocks.len(), 1);
        assert!(matches!(
            &blocks[0].term,
            BlockPyTerm::Jump(label) if label.as_str() == "target"
        ));
    }

    #[test]
    fn sequence_return_helper_emits_return_block() {
        let mut blocks = Vec::new();
        let context = Context::new(crate::passes::ast_to_ast::Options::for_test(), "");
        let entry = emit_sequence_return_block_with_expr_setup(
            &context,
            &mut blocks,
            BlockPyLabel::from("ret_label"),
            vec![py_stmt!("prefix = 0")],
            Some(py_expr!("value")),
            None,
        )
        .expect("sequence return helper should lower");

        assert_eq!(entry, "ret_label");
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0].term, BlockPyTerm::Return(_)));
    }

    #[test]
    fn sequence_raise_helper_emits_raise_block() {
        let mut blocks = Vec::new();
        let context = Context::new(crate::passes::ast_to_ast::Options::for_test(), "");
        let entry = emit_sequence_raise_block_with_expr_setup(
            &context,
            &mut blocks,
            BlockPyLabel::from("raise_label"),
            vec![py_stmt!("prefix = 0")],
            BlockPyRaise {
                exc: Some(py_expr!("exc").into()),
            },
            None,
        )
        .expect("sequence raise helper should lower");

        assert_eq!(entry, "raise_label");
        assert_eq!(blocks.len(), 1);
        assert!(matches!(
            blocks[0].term,
            BlockPyTerm::Raise(BlockPyRaise { exc: Some(_) })
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
        let ast::Stmt::If(if_stmt) = &module[0] else {
            panic!("expected if stmt");
        };
        let remaining = vec![module[1].clone()];
        let mut blocks = Vec::new();
        let mut calls = Vec::new();
        let context = Context::new(crate::passes::ast_to_ast::Options::for_test(), "");

        let entry = lower_if_stmt_sequence_from_stmt(
            &context,
            if_stmt.clone(),
            &remaining,
            RegionTargets::new("cont".to_string(), None),
            vec![py_stmt!("prefix = 0")],
            &mut blocks,
            BlockPyLabel::from("if_label"),
            &mut |stmts: &[Stmt], targets: RegionTargets, _blocks: &mut Vec<BlockPyBlock>| {
                calls.push((stmts.len(), targets.normal_cont.clone()));
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
        assert!(matches!(blocks[0].term, BlockPyTerm::IfTerm(_)));
    }

    #[test]
    fn while_stmt_helper_lowers_loop_and_else_via_callbacks() {
        let mut blocks = Vec::new();
        let body = vec![py_stmt!("x = 1")];
        let else_body = vec![py_stmt!("x = 2")];
        let remaining = vec![py_stmt!("x = 3")];
        let mut sequence_calls = Vec::new();
        let mut loop_calls = Vec::new();
        let context = Context::new(crate::passes::ast_to_ast::Options::for_test(), "");

        let entry = lower_while_stmt_sequence(
            &context,
            &mut blocks,
            BlockPyLabel::from("_dp_bb_loop_fn_0"),
            Some(BlockPyLabel::from("_dp_bb_loop_fn_1")),
            vec![py_stmt!("prefix = 0")],
            py_expr!("flag"),
            &body,
            &else_body,
            &remaining,
            RegionTargets::new("cont".to_string(), None),
            &mut |stmts: &[Stmt], targets: RegionTargets, _blocks: &mut Vec<BlockPyBlock>| {
                if let Some(loop_labels) = targets.loop_labels {
                    loop_calls.push((
                        stmts.len(),
                        targets.normal_cont.clone(),
                        loop_labels.break_label,
                    ));
                    "loop_body".to_string()
                } else {
                    sequence_calls.push((stmts.len(), targets.normal_cont.clone()));
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
        let ast::Stmt::While(while_stmt) = &module[0] else {
            panic!("expected while stmt");
        };
        let remaining = vec![module[1].clone()];
        let mut blocks = Vec::new();
        let mut sequence_calls = Vec::new();
        let mut loop_calls = Vec::new();
        let context = Context::new(crate::passes::ast_to_ast::Options::for_test(), "");

        let entry = lower_while_stmt_sequence_from_stmt(
            &context,
            while_stmt.clone(),
            &remaining,
            RegionTargets::new("cont".to_string(), None),
            vec![py_stmt!("prefix = 0")],
            &mut blocks,
            BlockPyLabel::from("_dp_bb_loop_fn_0"),
            Some(BlockPyLabel::from("_dp_bb_loop_fn_1")),
            &mut |stmts: &[Stmt], targets: RegionTargets, _blocks: &mut Vec<BlockPyBlock>| {
                if let Some(loop_labels) = targets.loop_labels {
                    loop_calls.push((
                        stmts.len(),
                        targets.normal_cont.clone(),
                        loop_labels.break_label,
                    ));
                    "loop_body".to_string()
                } else {
                    sequence_calls.push((stmts.len(), targets.normal_cont.clone()));
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
            vec![(1, "_dp_bb_loop_fn_0".to_string(), "seq_1".to_string())]
        );
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].label.as_str(), "_dp_bb_loop_fn_0");
        assert_eq!(blocks[1].label.as_str(), "_dp_bb_loop_fn_1");
    }

    #[test]
    fn lowers_generator_yield_from_to_explicit_blockpy_dispatch() {
        let blockpy = wrapped_core_blockpy(
            r#"
def gen(it):
    yield from it
"#,
        );
        let rendered = crate::block_py::pretty::blockpy_module_to_string(&blockpy);
        assert!(rendered.contains("branch_table"));
        assert!(rendered.contains("__dp_exception_matches"), "{rendered}");
        assert!(rendered.contains("yield_from_throw_lookup"), "{rendered}");
        assert!(rendered.contains("yield_from_except"), "{rendered}");
        assert!(
            !rendered.contains("__dp_generator_yield_from_step"),
            "{rendered}"
        );
        assert!(!rendered.contains("yield from it"), "{rendered}");
    }

    #[test]
    fn lowers_async_generator_yield_to_explicit_blockpy_dispatch() {
        let blockpy = wrapped_core_blockpy(
            r#"
async def agen(n):
    yield n
"#,
        );
        let rendered = crate::block_py::pretty::blockpy_module_to_string(&blockpy);
        assert!(rendered.contains("async_generator agen(n):"), "{rendered}");
        assert!(
            rendered.contains(
                "function agen(_dp_self, _dp_send_value, _dp_resume_exc, _dp_transport_sent):"
            ),
            "{rendered}"
        );
        assert!(
            rendered.contains("return __dp_make_closure_async_generator"),
            "{rendered}"
        );
        assert!(rendered.contains("branch_table"), "{rendered}");
        assert!(!rendered.contains("yield n"), "{rendered}");
    }

    #[test]
    fn lowers_coroutine_completion_outside_user_exception_region() {
        let blockpy = wrapped_core_blockpy(
            r#"
async def outer(inner):
    try:
        value = await inner()
        return ("ok", False)
    except Exception:
        return ("StopIteration", True)
"#,
        );
        let rendered = crate::block_py::pretty::blockpy_module_to_string(&blockpy);
        let resume = function_by_name(&blockpy, "outer_resume");
        let stop_iteration_raise_labels = resume
            .blocks
            .iter()
            .filter_map(|block| match &block.term {
                BlockPyTerm::Raise(BlockPyRaise {
                    exc: Some(CoreBlockPyExpr::Call(call)),
                }) if matches!(
                    call.func.as_ref(),
                    CoreBlockPyExpr::Name(name)
                        if name.id.as_str() == "StopIteration"
                ) =>
                {
                    Some(block.label.as_str().to_string())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            !stop_iteration_raise_labels.is_empty(),
            "missing synthetic StopIteration blocks in:\n{rendered}"
        );
        for label in stop_iteration_raise_labels {
            assert_eq!(
                lowered_exception_edges(&resume.blocks)
                    .get(label.as_str())
                    .cloned()
                    .flatten(),
                None,
                "synthetic completion should bypass user handlers for {label}:\n{rendered}"
            );
        }
    }

    #[test]
    fn lowers_assert_if_it_reaches_blockpy_stmt_lowering() {
        let module = ruff_python_parser::parse_module(
            r#"
def f(x):
    assert x
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = &module[0] else {
            panic!("expected function def");
        };
        let context = test_context();
        let mut out = crate::block_py::BlockPyCfgFragmentBuilder::<BlockPyStmt, BlockPyTerm>::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(&context, &func.body[0], &mut out, None, &mut next_label_id)
            .expect("assert lowering should succeed");
        let fragment = out.finish();
        assert!(matches!(fragment.body.as_slice(), [BlockPyStmt::If(_)]));
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
        let ast::Stmt::FunctionDef(func) = &module[0] else {
            panic!("expected function def");
        };
        lower_stmt_for_panic_test(&func.body[0]);
    }

    #[test]
    fn lowers_augassign_if_it_reaches_blockpy_stmt_lowering() {
        let module = ruff_python_parser::parse_module(
            r#"
def f(x):
    x += 1
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = &module[0] else {
            panic!("expected function def");
        };
        let context = test_context();
        let mut out = crate::block_py::BlockPyCfgFragmentBuilder::<BlockPyStmt, BlockPyTerm>::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(&context, &func.body[0], &mut out, None, &mut next_label_id)
            .expect("augassign lowering should succeed");
        let fragment = out.finish();
        assert!(matches!(fragment.body.as_slice(), [BlockPyStmt::Assign(_)]));
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
        let ast::Stmt::FunctionDef(func) = &module[0] else {
            panic!("expected function def");
        };
        lower_stmt_for_panic_test(&func.body[0]);
    }

    #[test]
    fn lowers_typealias_if_it_reaches_blockpy_stmt_lowering() {
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
        let context = test_context();
        let mut out = crate::block_py::BlockPyCfgFragmentBuilder::<BlockPyStmt, BlockPyTerm>::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(&context, &module[0], &mut out, None, &mut next_label_id)
            .expect("type alias lowering should succeed");
        let fragment = out.finish();
        assert!(!fragment.body.is_empty());
    }

    #[test]
    fn lowers_match_if_it_reaches_blockpy_stmt_lowering() {
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
        let ast::Stmt::FunctionDef(func) = &module[0] else {
            panic!("expected function def");
        };
        let context = test_context();
        let mut out = crate::block_py::BlockPyCfgFragmentBuilder::<BlockPyStmt, BlockPyTerm>::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(&context, &func.body[0], &mut out, None, &mut next_label_id)
            .expect("match lowering should succeed");
        let fragment = out.finish();
        assert!(!fragment.body.is_empty() || fragment.term.is_some());
    }

    #[test]
    fn lowers_plain_import_if_it_reaches_blockpy_stmt_lowering() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    import os
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = &module[0] else {
            panic!("expected function def");
        };
        let context = test_context();
        let mut out = crate::block_py::BlockPyCfgFragmentBuilder::<BlockPyStmt, BlockPyTerm>::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(&context, &func.body[0], &mut out, None, &mut next_label_id)
            .expect("import lowering should succeed");
        let fragment = out.finish();
        assert!(matches!(fragment.body.as_slice(), [BlockPyStmt::Assign(_)]));
    }

    #[test]
    fn lowers_importfrom_if_it_reaches_blockpy_stmt_lowering() {
        let module = ruff_python_parser::parse_module(
            r#"
def f():
    from math import sqrt
"#,
        )
        .unwrap()
        .into_syntax()
        .body;
        let ast::Stmt::FunctionDef(func) = &module[0] else {
            panic!("expected function def");
        };
        let context = test_context();
        let mut out = crate::block_py::BlockPyCfgFragmentBuilder::<BlockPyStmt, BlockPyTerm>::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(&context, &func.body[0], &mut out, None, &mut next_label_id)
            .expect("import-from lowering should succeed");
        let fragment = out.finish();
        assert!(!fragment.body.is_empty());
    }

    #[test]
    fn lowers_bare_raise_to_optional_blockpy_raise() {
        let blockpy = wrapped_blockpy(
            r#"
def f():
    raise
"#,
        );
        let raise_stmt = match &function_by_name(&blockpy, "f").blocks[0].term {
            BlockPyTerm::Raise(raise_stmt) => raise_stmt,
            other => panic!("expected BlockPy raise term, got {other:?}"),
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
        let ast::Stmt::FunctionDef(func) = &module[0] else {
            panic!("expected function def");
        };
        lower_stmt_for_panic_test(&func.body[0]);
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
        let ast::Stmt::While(while_stmt) = &module[0] else {
            panic!("expected while stmt");
        };
        let context = test_context();
        let mut out = crate::block_py::BlockPyCfgFragmentBuilder::<BlockPyStmt, BlockPyTerm>::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(
            &context,
            &Stmt::While(while_stmt.clone()),
            &mut out,
            None,
            &mut next_label_id,
        )
        .unwrap();
    }
}
