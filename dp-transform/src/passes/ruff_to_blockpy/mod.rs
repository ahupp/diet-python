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
    BlockPyCallableSemanticInfo, BlockPyEdge, BlockPyFallthroughTerm, BlockPyFunction,
    BlockPyFunctionKind, BlockPyLabel, BlockPyPass, BlockPyStmt, BlockPyTerm, CfgBlock,
    ClosureLayout, FunctionName, FunctionNameGen,
};
use crate::namegen::fresh_name;
use crate::passes::ast_to_ast::context::Context;
use crate::passes::ast_to_ast::expr_utils::make_tuple;
use crate::passes::RuffBlockPyPass;
use crate::ruff_ast_to_string;
use crate::template::is_simple;
use crate::transformer::{walk_expr, Transformer};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt};
use std::collections::{HashMap, HashSet};
mod bb_shape;
mod compat;
pub(crate) mod expr_lowering;
mod module_plan;
mod stmt_lowering;
mod stmt_sequences;
mod try_regions;

pub(crate) use super::blockpy_generators::build_blockpy_closure_layout;
pub(crate) use bb_shape::{
    lower_structured_located_blocks_to_bb_blocks, lowered_exception_edges,
    populate_exception_edge_args,
};
pub(crate) use module_plan::rewrite_ast_to_lowered_blockpy_module_plan_with_module;

pub(crate) use compat::{
    compat_block_from_blockpy, compat_block_from_blockpy_with_exc_target, emit_for_loop_blocks,
    emit_if_branch_block_with_expr_setup, emit_sequence_jump_block,
    emit_sequence_raise_block_with_expr_setup, emit_sequence_return_block_with_expr_setup,
    emit_simple_while_blocks_with_expr_setup,
};
pub(crate) use stmt_lowering::{
    build_for_target_assign_body, lower_star_try_stmt_sequence, lower_try_stmt_sequence,
    lower_with_stmt_sequence,
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
    #[derive(Default)]
    struct CellRefLogicalNameCollector {
        names: HashSet<String>,
    }

    impl Transformer for CellRefLogicalNameCollector {
        fn visit_expr(&mut self, expr: &mut Expr) {
            if let Expr::Call(call) = expr {
                if let Expr::Name(name) = call.func.as_ref() {
                    if name.id.as_str() == "__dp_cell_ref" {
                        if let Some(ast::Expr::StringLiteral(literal)) = call.arguments.args.first()
                        {
                            self.names.insert(literal.value.to_str().to_string());
                        }
                    }
                }
            }
            walk_expr(self, expr);
        }
    }

    fn collect_cell_ref_logical_names_in_stmt(stmt: &BlockPyStmt, out: &mut HashSet<String>) {
        match stmt {
            BlockPyStmt::Assign(assign) => {
                let mut collector = CellRefLogicalNameCollector::default();
                let mut expr = assign.value.clone();
                collector.visit_expr(&mut expr);
                out.extend(collector.names);
            }
            BlockPyStmt::Expr(expr) => {
                let mut collector = CellRefLogicalNameCollector::default();
                let mut expr = expr.clone();
                collector.visit_expr(&mut expr);
                out.extend(collector.names);
            }
            BlockPyStmt::Delete(_) => {}
            BlockPyStmt::If(if_stmt) => {
                let mut collector = CellRefLogicalNameCollector::default();
                let mut test = if_stmt.test.clone();
                collector.visit_expr(&mut test);
                out.extend(collector.names);
                collect_cell_ref_logical_names_in_fragment(&if_stmt.body, out);
                collect_cell_ref_logical_names_in_fragment(&if_stmt.orelse, out);
            }
        }
    }

    fn collect_cell_ref_logical_names_in_term(term: &BlockPyTerm, out: &mut HashSet<String>) {
        match term {
            BlockPyTerm::Jump(_) => {}
            BlockPyTerm::IfTerm(if_term) => {
                let mut collector = CellRefLogicalNameCollector::default();
                let mut test = if_term.test.clone();
                collector.visit_expr(&mut test);
                out.extend(collector.names);
            }
            BlockPyTerm::BranchTable(branch) => {
                let mut collector = CellRefLogicalNameCollector::default();
                let mut index = branch.index.clone();
                collector.visit_expr(&mut index);
                out.extend(collector.names);
            }
            BlockPyTerm::Raise(raise) => {
                let Some(exc) = raise.exc.as_ref() else {
                    return;
                };
                let mut collector = CellRefLogicalNameCollector::default();
                let mut exc = exc.clone();
                collector.visit_expr(&mut exc);
                out.extend(collector.names);
            }
            BlockPyTerm::Return(expr) => {
                let mut collector = CellRefLogicalNameCollector::default();
                let mut expr = expr.clone();
                collector.visit_expr(&mut expr);
                out.extend(collector.names);
            }
        }
    }

    fn collect_cell_ref_logical_names_in_fragment(
        fragment: &crate::block_py::BlockPyStmtFragment,
        out: &mut HashSet<String>,
    ) {
        for stmt in &fragment.body {
            collect_cell_ref_logical_names_in_stmt(stmt, out);
        }
        if let Some(term) = fragment.term.as_ref() {
            collect_cell_ref_logical_names_in_term(term, out);
        }
    }

    let param_names = callable_def.params.names();
    let owned_cell_slot_names = callable_def.semantic.owned_cell_storage_names();
    let mut local_cell_slots = owned_cell_slot_names.iter().cloned().collect::<Vec<_>>();
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
    let deleted_names: HashSet<String> = callable_def
        .blocks
        .iter()
        .flat_map(|block| block.body.iter())
        .filter_map(|stmt| match stmt {
            BlockPyStmt::Delete(delete) => Some(delete.target.id.to_string()),
            _ => None,
        })
        .collect();
    let mut cell_ref_logical_names = HashSet::new();
    for block in &callable_def.blocks {
        for stmt in &block.body {
            collect_cell_ref_logical_names_in_stmt(stmt, &mut cell_ref_logical_names);
        }
        collect_cell_ref_logical_names_in_term(&block.term, &mut cell_ref_logical_names);
    }
    let mut referenced_names = used_names
        .iter()
        .chain(defined_names.iter())
        .chain(deleted_names.iter())
        .chain(cell_ref_logical_names.iter())
        .cloned()
        .collect::<Vec<_>>();
    referenced_names.sort();
    referenced_names.dedup();
    let mut capture_names = referenced_names
        .iter()
        .filter(|name| !param_name_set.contains(name.as_str()))
        .filter(|name| {
            callable_def
                .semantic
                .resolved_load_binding_kind(name.as_str())
                == BlockPyBindingKind::Cell(crate::block_py::BlockPyCellBindingKind::Capture)
        })
        .cloned()
        .collect::<Vec<_>>();
    capture_names.extend(
        cell_ref_logical_names
            .iter()
            .filter(|logical_name| {
                !owned_cell_slot_names.contains(
                    callable_def
                        .semantic
                        .cell_capture_source_name(logical_name.as_str())
                        .as_str(),
                ) && !param_name_set.contains(logical_name.as_str())
            })
            .cloned(),
    );
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
        let logical_name = callable_def
            .semantic
            .logical_name_for_cell_storage(slot.as_str())
            .unwrap_or(slot);
        if !state_vars.iter().any(|existing| existing == &logical_name) {
            state_vars.push(logical_name);
        }
    }

    Some(build_blockpy_closure_layout(
        &callable_def.semantic,
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
    if matches!(callable_def.kind, BlockPyFunctionKind::Function) {
        rewrite_current_exception_placeholders_in_lowered_blocks(&mut callable_def.blocks);
    }
    callable_def.closure_layout =
        build_semantic_blockpy_closure_layout(&callable_def, &HashSet::new());
    callable_def
}

pub(crate) fn recompute_semantic_blockpy_closure_layout(
    callable_def: &BlockPyFunction<RuffBlockPyPass>,
) -> Option<ClosureLayout> {
    build_semantic_blockpy_closure_layout(callable_def, &HashSet::new())
}

struct CurrentExceptionPlaceholderRewriter<'a> {
    exc_name: &'a str,
}

impl Transformer for CurrentExceptionPlaceholderRewriter<'_> {
    fn visit_expr(&mut self, expr: &mut Expr) {
        if is_current_exception_placeholder_expr(expr) {
            *expr = current_exception_name_expr(self.exc_name);
            return;
        }
        if is_exc_info_placeholder_expr(expr) {
            *expr = current_exception_info_expr(self.exc_name);
            return;
        }
        walk_expr(self, expr);
    }
}

fn rewrite_current_exception_placeholders_in_lowered_blocks(
    blocks: &mut [crate::block_py::CfgBlock<BlockPyStmt, BlockPyTerm>],
) {
    for block in blocks {
        let Some(exc_name) = block.exception_param().map(ToString::to_string) else {
            continue;
        };
        for stmt in &mut block.body {
            rewrite_current_exception_placeholders_in_stmt(stmt, exc_name.as_str());
        }
        rewrite_current_exception_placeholders_in_term(&mut block.term, exc_name.as_str());
    }
}

fn rewrite_current_exception_placeholders_in_stmt(stmt: &mut BlockPyStmt, exc_name: &str) {
    match stmt {
        BlockPyStmt::Assign(assign) => {
            rewrite_current_exception_placeholders_in_expr(&mut assign.value, exc_name);
        }
        BlockPyStmt::Expr(expr) => {
            rewrite_current_exception_placeholders_in_expr(expr, exc_name);
        }
        BlockPyStmt::Delete(_) => {}
        BlockPyStmt::If(if_stmt) => {
            rewrite_current_exception_placeholders_in_expr(&mut if_stmt.test, exc_name);
            for stmt in &mut if_stmt.body.body {
                rewrite_current_exception_placeholders_in_stmt(stmt, exc_name);
            }
            if let Some(term) = if_stmt.body.term.as_mut() {
                rewrite_current_exception_placeholders_in_term(term, exc_name);
            }
            for stmt in &mut if_stmt.orelse.body {
                rewrite_current_exception_placeholders_in_stmt(stmt, exc_name);
            }
            if let Some(term) = if_stmt.orelse.term.as_mut() {
                rewrite_current_exception_placeholders_in_term(term, exc_name);
            }
        }
    }
}

fn rewrite_current_exception_placeholders_in_term(term: &mut BlockPyTerm, exc_name: &str) {
    match term {
        BlockPyTerm::Jump(_) => {}
        BlockPyTerm::IfTerm(if_term) => {
            rewrite_current_exception_placeholders_in_expr(&mut if_term.test, exc_name);
        }
        BlockPyTerm::BranchTable(branch) => {
            rewrite_current_exception_placeholders_in_expr(&mut branch.index, exc_name);
        }
        BlockPyTerm::Raise(raise_stmt) => {
            if let Some(exc) = raise_stmt.exc.as_mut() {
                rewrite_current_exception_placeholders_in_expr(exc, exc_name);
            } else {
                raise_stmt.exc = Some(current_exception_name_expr(exc_name));
            }
        }
        BlockPyTerm::Return(value) => {
            rewrite_current_exception_placeholders_in_expr(value, exc_name);
        }
    }
}

fn rewrite_current_exception_placeholders_in_expr(expr: &mut Expr, exc_name: &str) {
    let mut rewriter = CurrentExceptionPlaceholderRewriter { exc_name };
    rewriter.visit_expr(expr);
}

fn is_current_exception_placeholder_expr(expr: &Expr) -> bool {
    let Expr::Call(call) = expr else {
        return false;
    };
    call.arguments.args.is_empty()
        && call.arguments.keywords.is_empty()
        && matches!(call.func.as_ref(), Expr::Name(name) if name.id.as_str() == "__dp_current_exception")
}

fn is_exc_info_placeholder_expr(expr: &Expr) -> bool {
    let Expr::Call(call) = expr else {
        return false;
    };
    call.arguments.args.is_empty()
        && call.arguments.keywords.is_empty()
        && matches!(call.func.as_ref(), Expr::Name(name) if name.id.as_str() == "__dp_exc_info")
}

fn current_exception_name_expr(exc_name: &str) -> Expr {
    py_expr!("{name:id}", name = exc_name)
}

fn current_exception_info_expr(exc_name: &str) -> Expr {
    py_expr!("__dp_exc_info_from_exception({name:id})", name = exc_name)
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
mod test;
