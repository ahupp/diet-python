use super::block_py::cfg::{
    fold_constant_brif_blockpy, fold_jumps_to_trivial_none_return_blockpy,
    prune_unreachable_blockpy_blocks, relabel_blockpy_blocks, rename_blockpy_labels,
};
use super::block_py::dataflow::{analyze_blockpy_use_def, compute_block_params_blockpy};
use super::block_py::exception::{
    contains_return_stmt_in_body, contains_return_stmt_in_handlers,
    rewrite_region_returns_to_finally_blockpy,
};
use super::block_py::state::{
    collect_state_vars, sync_target_cells_stmts as sync_target_cells_stmts_shared,
};
use super::block_py::{
    assert_blockpy_block_normalized, BlockPyBlock, BlockPyBlockMeta, BlockPyCallableDef,
    BlockPyCallableFacts, BlockPyFunctionKind, BlockPyLabel, BlockPyTerm, BlockPyTryJump,
    ClosureLayout, FunctionId, FunctionName, ENTRY_BLOCK_LABEL,
};
use super::function_lowering::rewrite_deleted_name_loads;
use super::stmt_utils::flatten_stmt_boxes;
use crate::basic_block::ast_to_ast::context::Context;
use crate::basic_block::ast_to_ast::expr_utils::make_tuple;
use crate::basic_block::block_py::param_specs::ParamSpec;
use crate::namegen::fresh_name;
use crate::ruff_ast_to_string;
use crate::template::is_simple;
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt};
use std::cell::Cell;
use std::collections::{HashMap, HashSet};
mod compat;
pub(crate) mod expr_lowering;
mod stmt_lowering;
mod stmt_sequences;
mod try_regions;

pub(crate) use super::blockpy_generators::build_blockpy_closure_layout;

pub(crate) use super::block_py::TryRegionPlan;
pub(crate) use compat::{
    compat_block_from_blockpy, compat_next_label, compat_next_temp, emit_for_loop_blocks,
    emit_if_branch_block_with_expr_setup, emit_sequence_jump_block,
    emit_sequence_raise_block_with_expr_setup, emit_sequence_return_block_with_expr_setup,
    emit_simple_while_blocks_with_expr_setup,
};
pub(crate) use stmt_lowering::{
    build_for_target_assign_body, lower_star_try_stmt_sequence, lower_stmt_into,
    lower_try_stmt_sequence, lower_with_stmt_sequence, rewrite_assign_stmt, rewrite_augassign_stmt,
    rewrite_delete_stmt, rewrite_type_alias_stmt,
};
pub(crate) use stmt_sequences::{
    lower_expanded_stmt_sequence, lower_stmt_sequence_with_state, lower_stmts_to_blockpy_stmts,
};
pub(crate) use try_regions::{
    block_references_label, build_try_plan, finalize_try_regions, lower_try_regions,
    prepare_except_body, prepare_finally_body, TryPlan,
};

#[derive(Debug, Clone, Default)]
pub struct LoweredBlockPyExtra {
    pub block_params: HashMap<String, Vec<String>>,
    pub exception_edges: HashMap<String, Option<String>>,
}

pub type LoweredBlockPyFunctionWith<E, B> = BlockPyCallableDef<E, B, LoweredBlockPyExtra>;
pub type LoweredBlockPyFunction = LoweredBlockPyFunctionWith<Expr, BlockPyBlock<Expr>>;

impl<E, B> BlockPyCallableDef<E, B, LoweredBlockPyExtra> {
    pub fn block_params(&self) -> &HashMap<String, Vec<String>> {
        &self.extra.block_params
    }

    pub fn exception_edges(&self) -> &HashMap<String, Option<String>> {
        &self.extra.exception_edges
    }
}

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

pub(crate) fn build_blockpy_function<X>(
    function_id: FunctionId,
    names: FunctionName,
    params: ParamSpec,
    param_defaults: Vec<Expr>,
    doc: Option<String>,
    kind: BlockPyFunctionKind,
    entry_label: String,
    closure_layout: Option<ClosureLayout>,
    facts: BlockPyCallableFacts,
    try_regions: Vec<TryRegionPlan>,
    mut blocks: Vec<BlockPyBlock<Expr>>,
    extra: X,
) -> BlockPyCallableDef<Expr, BlockPyBlock<Expr>, X> {
    move_blockpy_entry_block_to_front(&mut blocks, entry_label.as_str());
    for block in &blocks {
        assert_blockpy_block_normalized(block);
    }
    BlockPyCallableDef {
        function_id,
        names,
        kind,
        params,
        param_defaults,
        blocks,
        doc,
        closure_layout,
        facts,
        try_regions,
        extra,
    }
}

fn move_blockpy_entry_block_to_front(blocks: &mut Vec<BlockPyBlock<Expr>>, entry_label: &str) {
    if let Some(entry_index) = blocks
        .iter()
        .position(|block| block.label.as_str() == entry_label)
    {
        if entry_index != 0 {
            let entry_block = blocks.remove(entry_index);
            blocks.insert(0, entry_block);
        }
    }
}

pub(crate) fn take_next_function_id(next_function_id: &mut usize) -> FunctionId {
    let id = FunctionId(*next_function_id);
    *next_function_id += 1;
    id
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_lowered_blockpy_function(
    callable_def: BlockPyCallableDef<Expr>,
    block_params: HashMap<String, Vec<String>>,
    exception_edges: HashMap<String, Option<String>>,
) -> LoweredBlockPyFunction {
    callable_def.map_extra(|_| LoweredBlockPyExtra {
        block_params,
        exception_edges,
    })
}

fn fresh_normalized_entry_collision_label(
    blocks: &[BlockPyBlock<Expr>],
    rename: &HashMap<String, String>,
) -> String {
    let existing = blocks
        .iter()
        .map(|block| block.label.as_str().to_string())
        .collect::<HashSet<_>>();
    let mut next_id = 0usize;
    loop {
        let candidate = format!("{ENTRY_BLOCK_LABEL}_{next_id}");
        if !existing.contains(candidate.as_str())
            && !rename.values().any(|value| value == &candidate)
        {
            return candidate;
        }
        next_id += 1;
    }
}

fn rename_label_map_keys<T: Clone>(
    map: &HashMap<String, T>,
    rename: &HashMap<String, String>,
) -> HashMap<String, T> {
    map.iter()
        .map(|(label, value)| {
            (
                rename
                    .get(label.as_str())
                    .cloned()
                    .unwrap_or_else(|| label.clone()),
                value.clone(),
            )
        })
        .collect()
}

fn rename_exception_edges(
    edges: &HashMap<String, Option<String>>,
    rename: &HashMap<String, String>,
) -> HashMap<String, Option<String>> {
    edges
        .iter()
        .map(|(label, target)| {
            let rewritten_label = rename
                .get(label.as_str())
                .cloned()
                .unwrap_or_else(|| label.clone());
            let rewritten_target = target.as_ref().map(|name| {
                rename
                    .get(name.as_str())
                    .cloned()
                    .unwrap_or_else(|| name.clone())
            });
            (rewritten_label, rewritten_target)
        })
        .collect()
}

pub(crate) fn normalize_exported_entry_block(
    entry_label: String,
    mut blocks: Vec<BlockPyBlock<Expr>>,
    block_params: HashMap<String, Vec<String>>,
    exception_edges: HashMap<String, Option<String>>,
) -> (
    Vec<BlockPyBlock<Expr>>,
    HashMap<String, Vec<String>>,
    HashMap<String, Option<String>>,
) {
    let mut rename = HashMap::new();
    if entry_label != ENTRY_BLOCK_LABEL {
        if blocks
            .iter()
            .any(|block| block.label.as_str() == ENTRY_BLOCK_LABEL)
        {
            rename.insert(
                ENTRY_BLOCK_LABEL.to_string(),
                fresh_normalized_entry_collision_label(&blocks, &rename),
            );
        }
        rename.insert(entry_label.clone(), ENTRY_BLOCK_LABEL.to_string());
    }

    if !rename.is_empty() {
        rename_blockpy_labels(&rename, &mut blocks);
    }

    if let Some(entry_index) = blocks
        .iter()
        .position(|block| block.label.as_str() == ENTRY_BLOCK_LABEL)
    {
        if entry_index != 0 {
            let entry_block = blocks.remove(entry_index);
            blocks.insert(0, entry_block);
        }
    }

    (
        blocks,
        rename_label_map_keys(&block_params, &rename),
        rename_exception_edges(&exception_edges, &rename),
    )
}

fn build_semantic_blockpy_closure_layout(
    callable_def: &BlockPyCallableDef<Expr>,
    injected_exception_names: &HashSet<String>,
) -> Option<ClosureLayout> {
    let entry_liveins = callable_def.entry_liveins();
    let param_names = callable_def.params.names();
    let mut local_cell_slots = callable_def
        .facts
        .cell_slots
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    local_cell_slots.sort();
    let param_name_set = param_names.iter().cloned().collect::<HashSet<_>>();
    let locally_assigned: HashSet<String> = callable_def
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
                    && !callable_def.facts.cell_slots.contains(name.as_str()))
                || (!name.starts_with("_dp_") && !locally_assigned.contains(name.as_str()))
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

    let mut state_vars = entry_liveins.to_vec();
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

pub(crate) fn build_lowered_blockpy_function_bundle(
    callable_def: BlockPyCallableDef<Expr>,
) -> LoweredBlockPyFunction {
    let callable_facts = callable_def.facts.clone();
    let param_names = callable_def.params.names();
    let mut callable_def = callable_def;
    if !callable_facts.deleted_names.is_empty() {
        rewrite_deleted_name_loads(
            &mut callable_def.blocks,
            &callable_facts.deleted_names,
            &callable_facts.unbound_local_names,
        );
    } else if !callable_facts.unbound_local_names.is_empty() {
        rewrite_deleted_name_loads(
            &mut callable_def.blocks,
            &HashSet::new(),
            &callable_facts.unbound_local_names,
        );
    }
    let mut blockpy_function = callable_def;
    let entry_label = blockpy_function.entry_label().to_string();
    let exception_edges = compute_blockpy_exception_edges(&blockpy_function.try_regions);
    let mut extra_successors = build_try_extra_successors(&blockpy_function.try_regions);
    let blocks_for_dataflow = std::mem::take(&mut blockpy_function.blocks);

    let mut state_vars = collect_state_vars(&param_names, &blocks_for_dataflow);
    for block in &blocks_for_dataflow {
        let Some(exc_param) = block.meta.exc_param.as_ref() else {
            continue;
        };
        if !state_vars.iter().any(|existing| existing == exc_param) {
            state_vars.push(exc_param.clone());
        }
    }

    for (source, target) in &exception_edges {
        let Some(target) = target.as_ref() else {
            continue;
        };
        let successors = extra_successors.entry(source.clone()).or_default();
        if !successors.iter().any(|existing| existing == target) {
            successors.push(target.clone());
        }
    }
    let mut block_params =
        compute_block_params_blockpy(&blocks_for_dataflow, &state_vars, &extra_successors);
    for block in &blocks_for_dataflow {
        let Some(exc_param) = block.meta.exc_param.as_ref() else {
            continue;
        };
        let params = block_params
            .entry(block.label.as_str().to_string())
            .or_default();
        if !params.iter().any(|existing| existing == exc_param) {
            params.push(exc_param.clone());
        }
    }
    let semantic_closure_layout = blockpy_function.closure_layout.clone();

    let function_id = blockpy_function.function_id;
    let names = blockpy_function.names.clone();
    let doc = blockpy_function.doc.clone();
    let params = blockpy_function.params.clone();
    let param_defaults = blockpy_function.param_defaults.clone();
    let (normalized_main_blocks, normalized_main_block_params, normalized_main_exception_edges) =
        normalize_exported_entry_block(
            entry_label,
            blocks_for_dataflow,
            block_params,
            exception_edges,
        );
    let main_function = build_blockpy_function(
        function_id,
        names,
        params,
        param_defaults,
        doc,
        BlockPyFunctionKind::Function,
        ENTRY_BLOCK_LABEL.to_string(),
        semantic_closure_layout,
        blockpy_function.facts.clone(),
        blockpy_function.try_regions.clone(),
        normalized_main_blocks,
        (),
    );
    build_lowered_blockpy_function(
        main_function,
        normalized_main_block_params,
        normalized_main_exception_edges,
    )
}

pub(crate) fn build_finalized_blockpy_callable_def(
    function_id: FunctionId,
    names: FunctionName,
    params: ParamSpec,
    param_defaults: Vec<Expr>,
    doc: Option<String>,
    kind: BlockPyFunctionKind,
    blocks: Vec<BlockPyBlock<Expr>>,
    try_regions: Vec<TryRegionPlan>,
    entry_label: String,
    end_label: String,
    facts: BlockPyCallableFacts,
) -> BlockPyCallableDef<Expr> {
    let callable_def = build_blockpy_function(
        function_id,
        names,
        params,
        param_defaults,
        doc,
        kind,
        entry_label.clone(),
        None,
        facts,
        try_regions,
        blocks,
        (),
    );
    finalize_blockpy_callable_def(callable_def, entry_label, end_label)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_blockpy_callable_def_from_runtime_input<FTemp>(
    context: &Context,
    function_id: FunctionId,
    names: FunctionName,
    params: ParamSpec,
    param_defaults: Vec<Expr>,
    runtime_input_body: &[Box<Stmt>],
    doc: Option<String>,
    end_label: String,
    blockpy_kind: BlockPyFunctionKind,
    facts: &BlockPyCallableFacts,
    next_block_id: &mut usize,
    next_temp: &mut FTemp,
) -> BlockPyCallableDef<Expr>
where
    FTemp: FnMut(&str, &mut usize) -> String,
{
    let mut blocks = Vec::new();
    let mut try_regions = Vec::new();
    let entry_label = lower_stmt_sequence_with_state(
        context,
        names.fn_name.as_str(),
        runtime_input_body,
        end_label.clone(),
        None,
        None,
        &mut blocks,
        &facts.cell_slots,
        &facts.outer_scope_names,
        &mut try_regions,
        next_block_id,
        next_temp,
    );
    build_finalized_blockpy_callable_def(
        function_id,
        names,
        params,
        param_defaults,
        doc,
        blockpy_kind,
        blocks,
        try_regions,
        entry_label,
        end_label,
        facts.clone(),
    )
}

pub(crate) fn build_try_extra_successors(
    try_regions: &[TryRegionPlan],
) -> HashMap<String, Vec<String>> {
    let mut extra = HashMap::new();
    for region in try_regions {
        for label in &region.body_region_labels {
            extra
                .entry(label.clone())
                .or_insert_with(Vec::new)
                .push(region.body_exception_target.clone());
        }
        if let Some(cleanup_target) = region.cleanup_exception_target.as_ref() {
            for label in &region.cleanup_region_labels {
                extra
                    .entry(label.clone())
                    .or_insert_with(Vec::new)
                    .push(cleanup_target.clone());
            }
        }
    }
    extra
}

pub(crate) fn compute_blockpy_exception_edges(
    try_regions: &[TryRegionPlan],
) -> HashMap<String, Option<String>> {
    let mut exception_edges = HashMap::new();
    for region in try_regions {
        let body_rank = region.body_region_labels.len();
        for label in &region.body_region_labels {
            update_try_edge_if_better(
                &mut exception_edges,
                label.clone(),
                body_rank,
                Some(region.body_exception_target.clone()),
            );
        }
        if let Some(cleanup_target) = region.cleanup_exception_target.as_ref() {
            let cleanup_rank = region.cleanup_region_labels.len();
            for label in &region.cleanup_region_labels {
                update_try_edge_if_better(
                    &mut exception_edges,
                    label.clone(),
                    cleanup_rank,
                    Some(cleanup_target.clone()),
                );
            }
        }
    }
    exception_edges
        .into_iter()
        .map(|(label, (_rank, target))| (label, target))
        .collect::<HashMap<_, _>>()
}

fn update_try_edge_if_better(
    edges: &mut HashMap<String, (usize, Option<String>)>,
    label: String,
    rank: usize,
    target: Option<String>,
) {
    let should_update = match edges.get(label.as_str()) {
        Some((existing_rank, _)) => rank < *existing_rank,
        None => true,
    };
    if should_update {
        edges.insert(label, (rank, target));
    }
}

fn relabel_try_regions(try_regions: &mut [TryRegionPlan], rename: &HashMap<String, String>) {
    for region in try_regions {
        for label in &mut region.body_region_labels {
            if let Some(rewritten) = rename.get(label.as_str()) {
                *label = rewritten.clone();
            }
        }
        if let Some(rewritten) = rename.get(region.body_exception_target.as_str()) {
            region.body_exception_target = rewritten.clone();
        }
        for label in &mut region.cleanup_region_labels {
            if let Some(rewritten) = rename.get(label.as_str()) {
                *label = rewritten.clone();
            }
        }
        if let Some(target) = region.cleanup_exception_target.as_mut() {
            if let Some(rewritten) = rename.get(target.as_str()) {
                *target = rewritten.clone();
            }
        }
    }
}

pub(crate) fn finalize_blockpy_callable_def(
    mut callable_def: BlockPyCallableDef<Expr>,
    mut entry_label: String,
    end_label: String,
) -> BlockPyCallableDef<Expr> {
    let needs_end_block = entry_label == end_label
        || callable_def
            .blocks
            .iter()
            .any(|block| block_references_label(block, end_label.as_str()));
    if needs_end_block {
        callable_def.blocks.push(BlockPyBlock {
            label: BlockPyLabel::from(end_label),
            body: Vec::new(),
            term: BlockPyTerm::Return(None),
            meta: BlockPyBlockMeta::default(),
        });
    }
    fold_jumps_to_trivial_none_return_blockpy(&mut callable_def.blocks);
    fold_constant_brif_blockpy(&mut callable_def.blocks);
    prune_unreachable_blockpy_blocks(entry_label.as_str(), &[], &mut callable_def.blocks);
    let (relabelled_entry_label, label_rename) =
        relabel_blockpy_blocks("_dp_bb", entry_label.as_str(), &mut callable_def.blocks);
    entry_label = relabelled_entry_label;
    relabel_try_regions(&mut callable_def.try_regions, &label_rename);
    move_blockpy_entry_block_to_front(&mut callable_def.blocks, entry_label.as_str());
    callable_def.closure_layout =
        build_semantic_blockpy_closure_layout(&callable_def, &HashSet::new());
    callable_def
}

#[derive(Clone)]
pub(crate) struct LoopContext {
    continue_label: BlockPyLabel,
    break_label: BlockPyLabel,
}

fn assign_delete_error(message: &str, stmt: &Stmt) -> String {
    format!("{message}\nstmt:\n{}", ruff_ast_to_string(stmt).trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::basic_block::ast_to_ast::{context::Context, Options};
    use crate::basic_block::block_py::{
        BlockPyCallableDef, BlockPyModule, BlockPyRaise, BlockPyStmt, BlockPyTerm,
    };
    use crate::basic_block::ruff_to_blockpy::stmt_sequences::{
        lower_for_stmt_sequence, lower_if_stmt_sequence, lower_if_stmt_sequence_from_stmt,
        lower_while_stmt_sequence, lower_while_stmt_sequence_from_stmt, plan_stmt_sequence_head,
    };
    use crate::basic_block::ruff_to_blockpy::try_regions::build_try_plan;
    use crate::{transform_str_to_blockpy_with_options, transform_str_to_ruff_with_options};
    use ruff_python_ast::Expr;

    fn wrapped_blockpy(source: &str) -> BlockPyModule<Expr> {
        transform_str_to_blockpy_with_options(source, Options::for_test()).unwrap()
    }

    fn wrapped_semantic_blockpy(source: &str) -> BlockPyModule<Expr> {
        transform_str_to_ruff_with_options(source, Options::for_test())
            .unwrap()
            .get_pass::<BlockPyModule>("semantic_blockpy")
            .cloned()
            .expect("semantic_blockpy pass should be tracked")
    }

    fn function_by_name<'a>(blockpy: &'a BlockPyModule, bind_name: &str) -> &'a BlockPyCallableDef {
        blockpy
            .callable_defs
            .iter()
            .find(|func| func.names.bind_name == bind_name)
            .unwrap_or_else(|| panic!("missing BlockPy function {bind_name}; got {blockpy:?}"))
    }

    fn lower_stmt_for_panic_test(stmt: &Stmt) {
        let context = Context::new(Options::for_test(), "");
        let mut out = crate::basic_block::block_py::BlockPyCfgFragmentBuilder::<
            BlockPyStmt,
            BlockPyTerm,
        >::new();
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
        let rendered = crate::basic_block::block_py::pretty::blockpy_module_to_string(&blockpy);
        assert!(blocks
            .iter()
            .any(|block| matches!(block.term, BlockPyTerm::IfTerm(_))));
        assert!(rendered.contains("try_jump"), "{rendered}");
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
        let rendered = crate::basic_block::block_py::pretty::blockpy_module_to_string(&blockpy);
        assert!(
            rendered.contains("await __dp_anext_or_sentinel"),
            "{rendered}"
        );
        assert!(rendered.contains("__dp_anext_or_sentinel"), "{rendered}");
    }

    #[test]
    fn lowers_generator_yield_to_explicit_blockpy_dispatch() {
        let blockpy = wrapped_blockpy(
            r#"
def gen(n):
    yield n
"#,
        );
        let rendered = crate::basic_block::blockpy_module_to_string(&blockpy);
        assert!(rendered.contains("generator gen(n):"), "{rendered}");
        assert!(rendered.contains("function gen(n):"), "{rendered}");
        assert!(rendered.contains("branch_table"));
        assert!(!rendered.contains("yield n"));
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
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = crate::basic_block::ast_to_ast::body::stmt_ref(&func.body, 0);

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
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = crate::basic_block::ast_to_ast::body::stmt_ref(&func.body, 0);

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
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = crate::basic_block::ast_to_ast::body::stmt_ref(&func.body, 0);

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
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = crate::basic_block::ast_to_ast::body::stmt_ref(&func.body, 0);

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
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = crate::basic_block::ast_to_ast::body::stmt_ref(&func.body, 0);

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
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = crate::basic_block::ast_to_ast::body::stmt_ref(&func.body, 0);

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
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let stmt = crate::basic_block::ast_to_ast::body::stmt_ref(&func.body, 0);

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
        let rendered = crate::basic_block::blockpy_module_to_string(&blockpy);
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
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let ast::Stmt::For(for_stmt) =
            crate::basic_block::ast_to_ast::body::stmt_ref(&func.body, 0)
        else {
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
    fn lower_with_stmt_sequence_emits_shared_cleanup_try_jump() {
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
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let ast::Stmt::With(with_stmt) =
            crate::basic_block::ast_to_ast::body::stmt_ref(&func.body, 0)
        else {
            panic!("expected with stmt");
        };

        let mut blocks = Vec::new();
        let next_block_id = Cell::new(0usize);
        let (entry, try_region) = lower_with_stmt_sequence(
            "demo",
            with_stmt.clone(),
            &[],
            "cont".to_string(),
            Vec::new(),
            &mut blocks,
            &HashSet::new(),
            &next_block_id,
            false,
            &mut |_expanded, cont_label, _blocks| {
                assert!(cont_label == "cont" || cont_label.ends_with("__normal"));
                cont_label
            },
        );

        assert!(!entry.is_empty());
        assert!(try_region.is_some());
        assert!(blocks
            .iter()
            .any(|block| matches!(block.term, BlockPyTerm::TryJump(_))));
        assert!(!blocks
            .iter()
            .flat_map(|block| block.body.iter())
            .any(|stmt| {
                matches!(stmt,
                    BlockPyStmt::Assign(assign)
                        if assign.target.id.as_str().contains("with_ok")
                            || assign.target.id.as_str().contains("with_suppress")
                )
            }));
    }

    #[test]
    fn lower_try_stmt_sequence_emits_try_jump() {
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
        let ast::Stmt::Try(try_stmt) =
            crate::basic_block::ast_to_ast::body::stmt_ref(&func.body, 0)
        else {
            panic!("expected try stmt");
        };

        let mut blocks = Vec::new();
        let mut next_label_id = 0usize;
        let try_plan = build_try_plan("demo", false, false, &mut next_label_id);
        let (entry, try_region) = lower_try_stmt_sequence(
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
        assert_eq!(try_region.body_exception_target, "cont");
        assert!(blocks
            .iter()
            .any(|block| matches!(block.term, BlockPyTerm::TryJump(_))));
    }

    #[test]
    fn expanded_stmt_helper_returns_expanded_entry_without_linear_prefix() {
        let mut blocks = Vec::new();
        let mut saw_expanded = false;
        let entry = lower_expanded_stmt_sequence(
            vec![py_stmt!("pass")],
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
            vec![py_stmt!("pass")],
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
            &blocks[0].term,
            BlockPyTerm::Jump(label) if label.as_str() == "expanded_entry"
        ));
    }

    #[test]
    fn if_stmt_helper_lowers_both_branches_via_callback() {
        let mut blocks = Vec::new();
        let then_body = vec![Box::new(py_stmt!("x = 1"))];
        let else_body = vec![Box::new(py_stmt!("x = 2"))];
        let mut calls = Vec::new();
        let context = Context::new(crate::basic_block::ast_to_ast::Options::for_test(), "");

        let entry = lower_if_stmt_sequence(
            &context,
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
        assert!(matches!(blocks[0].term, BlockPyTerm::IfTerm(_)));
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
            &blocks[0].term,
            BlockPyTerm::Jump(label) if label.as_str() == "target"
        ));
    }

    #[test]
    fn sequence_return_helper_emits_return_block() {
        let mut blocks = Vec::new();
        let context = Context::new(crate::basic_block::ast_to_ast::Options::for_test(), "");
        let entry = emit_sequence_return_block_with_expr_setup(
            &context,
            &mut blocks,
            "ret_label".to_string(),
            vec![py_stmt!("prefix = 0")],
            Some(py_expr!("value")),
        )
        .expect("sequence return helper should lower");

        assert_eq!(entry, "ret_label");
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0].term, BlockPyTerm::Return(Some(_))));
    }

    #[test]
    fn sequence_raise_helper_emits_raise_block() {
        let mut blocks = Vec::new();
        let context = Context::new(crate::basic_block::ast_to_ast::Options::for_test(), "");
        let entry = emit_sequence_raise_block_with_expr_setup(
            &context,
            &mut blocks,
            "raise_label".to_string(),
            vec![py_stmt!("prefix = 0")],
            BlockPyRaise {
                exc: Some(py_expr!("exc").into()),
            },
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
        let ast::Stmt::If(if_stmt) = module.body[0].as_ref() else {
            panic!("expected if stmt");
        };
        let remaining = vec![module.body[1].clone()];
        let mut blocks = Vec::new();
        let mut calls = Vec::new();
        let context = Context::new(crate::basic_block::ast_to_ast::Options::for_test(), "");

        let entry = lower_if_stmt_sequence_from_stmt(
            &context,
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
        assert!(matches!(blocks[0].term, BlockPyTerm::IfTerm(_)));
    }

    #[test]
    fn while_stmt_helper_lowers_loop_and_else_via_callbacks() {
        let mut blocks = Vec::new();
        let body = vec![Box::new(py_stmt!("x = 1"))];
        let else_body = vec![Box::new(py_stmt!("x = 2"))];
        let remaining = vec![Box::new(py_stmt!("x = 3"))];
        let mut sequence_calls = Vec::new();
        let mut loop_calls = Vec::new();
        let context = Context::new(crate::basic_block::ast_to_ast::Options::for_test(), "");

        let entry = lower_while_stmt_sequence(
            &context,
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
        let context = Context::new(crate::basic_block::ast_to_ast::Options::for_test(), "");

        let entry = lower_while_stmt_sequence_from_stmt(
            &context,
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
            vec![(1, "_dp_bb_loop_fn_0".to_string(), "seq_1".to_string())]
        );
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].label.as_str(), "_dp_bb_loop_fn_0");
        assert_eq!(blocks[1].label.as_str(), "_dp_bb_loop_fn_1");
    }

    #[test]
    fn lowers_generator_yield_from_to_explicit_blockpy_dispatch() {
        let blockpy = wrapped_blockpy(
            r#"
def gen(it):
    yield from it
"#,
        );
        let rendered = crate::basic_block::blockpy_module_to_string(&blockpy);
        assert!(rendered.contains("branch_table"));
        assert!(!rendered.contains("yield from it"));
    }

    #[test]
    fn lowers_async_generator_yield_to_explicit_blockpy_dispatch() {
        let blockpy = wrapped_blockpy(
            r#"
async def agen(n):
    yield n
"#,
        );
        let rendered = crate::basic_block::blockpy_module_to_string(&blockpy);
        assert!(rendered.contains("branch_table"));
        assert!(!rendered.contains("yield n"));
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
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let context = test_context();
        let mut out = crate::basic_block::block_py::BlockPyCfgFragmentBuilder::<
            BlockPyStmt,
            BlockPyTerm,
        >::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(
            &context,
            crate::basic_block::ast_to_ast::body::stmt_ref(&func.body, 0),
            &mut out,
            None,
            &mut next_label_id,
        )
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
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        lower_stmt_for_panic_test(crate::basic_block::ast_to_ast::body::stmt_ref(
            &func.body, 0,
        ));
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
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let context = test_context();
        let mut out = crate::basic_block::block_py::BlockPyCfgFragmentBuilder::<
            BlockPyStmt,
            BlockPyTerm,
        >::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(
            &context,
            crate::basic_block::ast_to_ast::body::stmt_ref(&func.body, 0),
            &mut out,
            None,
            &mut next_label_id,
        )
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
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        lower_stmt_for_panic_test(crate::basic_block::ast_to_ast::body::stmt_ref(
            &func.body, 0,
        ));
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
        let mut out = crate::basic_block::block_py::BlockPyCfgFragmentBuilder::<
            BlockPyStmt,
            BlockPyTerm,
        >::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(
            &context,
            module.body[0].as_ref(),
            &mut out,
            None,
            &mut next_label_id,
        )
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
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let context = test_context();
        let mut out = crate::basic_block::block_py::BlockPyCfgFragmentBuilder::<
            BlockPyStmt,
            BlockPyTerm,
        >::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(
            &context,
            crate::basic_block::ast_to_ast::body::stmt_ref(&func.body, 0),
            &mut out,
            None,
            &mut next_label_id,
        )
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
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let context = test_context();
        let mut out = crate::basic_block::block_py::BlockPyCfgFragmentBuilder::<
            BlockPyStmt,
            BlockPyTerm,
        >::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(
            &context,
            crate::basic_block::ast_to_ast::body::stmt_ref(&func.body, 0),
            &mut out,
            None,
            &mut next_label_id,
        )
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
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        let context = test_context();
        let mut out = crate::basic_block::block_py::BlockPyCfgFragmentBuilder::<
            BlockPyStmt,
            BlockPyTerm,
        >::new();
        let mut next_label_id = 0usize;
        lower_stmt_into(
            &context,
            crate::basic_block::ast_to_ast::body::stmt_ref(&func.body, 0),
            &mut out,
            None,
            &mut next_label_id,
        )
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
        let ast::Stmt::FunctionDef(func) = module.body[0].as_ref() else {
            panic!("expected function def");
        };
        lower_stmt_for_panic_test(crate::basic_block::ast_to_ast::body::stmt_ref(
            &func.body, 0,
        ));
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
        let context = test_context();
        let mut out = crate::basic_block::block_py::BlockPyCfgFragmentBuilder::<
            BlockPyStmt,
            BlockPyTerm,
        >::new();
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
