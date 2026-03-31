use crate::block_py::cfg::{
    fold_constant_brif_blockpy, fold_jumps_to_trivial_none_return_blockpy,
    linearize_structured_ifs, prune_unreachable_blockpy_blocks,
};
use crate::block_py::exception::{
    contains_return_stmt_in_body, contains_return_stmt_in_handlers,
    rewrite_region_returns_to_finally_blockpy,
};
use crate::block_py::param_specs::ParamSpec;
use crate::block_py::{
    assert_blockpy_block_normalized, convert_blockpy_term_expr, move_entry_block_to_front,
    BlockPyCallableSemanticInfo, BlockPyEdge, BlockPyFallthroughTerm, BlockPyFunction,
    BlockPyFunctionKind, BlockPyLabel, BlockPyModule, BlockPyPass, BlockPyStmt, BlockPyTerm,
    CfgBlock, FunctionName, FunctionNameGen, RuffExpr, StructuredBlockPyStmt,
};
use crate::namegen::fresh_name;
use crate::passes::ast_to_ast::context::Context;
use crate::passes::ast_to_ast::expr_utils::make_tuple;
use crate::passes::blockpy_expr_simplify::simplify_blockpy_callable_def_exprs;
use crate::passes::{CoreBlockPyPassWithAwaitAndYield, RuffBlockPyPass};
use crate::ruff_ast_to_string;
use crate::template::is_simple;
use crate::transformer::{walk_expr, Transformer};
use crate::{py_expr, py_stmt};
use ruff_python_ast::{self as ast, Expr, Stmt};
use std::collections::HashMap;
mod bb_shape;
mod compat;
pub(crate) mod expr_lowering;
mod module_plan;
mod stmt_lowering;
mod stmt_sequences;
mod try_regions;

#[cfg(test)]
pub(crate) use bb_shape::lower_structured_located_blocks_to_bb_blocks;
pub(crate) use bb_shape::{
    lower_structured_blocks_to_bb_blocks, lowered_exception_edges, populate_exception_edge_args,
    rewrite_current_exception_in_core_blocks,
};
pub(crate) use module_plan::{
    rewrite_ast_to_core_blockpy_module_plan_with_module,
    rewrite_ast_to_lowered_blockpy_module_plan_with_module,
};

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

pub(crate) type LoweredBlockPyBlock<E = Expr> = CfgBlock<StructuredBlockPyStmt<E>, BlockPyTerm<E>>;
pub(crate) type BlockPyBlock<E = Expr> = LoweredBlockPyBlock<E>;

pub(crate) fn lower_blockpy_module_exprs_to_core(
    module: BlockPyModule<RuffBlockPyPass>,
) -> BlockPyModule<CoreBlockPyPassWithAwaitAndYield> {
    module.map_callable_defs(simplify_blockpy_callable_def_exprs)
}

pub(crate) fn rewrite_ast_to_core_blockpy_module_with_module(
    context: &Context,
    module: Vec<Stmt>,
    semantic_state: &crate::passes::ast_to_ast::semantic::SemanticAstState,
    module_name_gen: crate::block_py::ModuleNameGen,
) -> BlockPyModule<CoreBlockPyPassWithAwaitAndYield> {
    rewrite_ast_to_core_blockpy_module_plan_with_module(
        context,
        module,
        semantic_state,
        module_name_gen,
    )
}

#[derive(Debug, Clone)]
struct StructuredRuffBlockPyPass;

impl BlockPyPass for StructuredRuffBlockPyPass {
    type Name = ast::ExprName;
    type Expr = Expr;
    type Stmt = StructuredBlockPyStmt<Self::Expr>;
}

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

pub(crate) fn attach_exception_edges_to_blocks<S, E>(
    blocks: Vec<CfgBlock<S, BlockPyTerm<E>>>,
    exception_edges: &HashMap<BlockPyLabel, Option<BlockPyLabel>>,
) -> Vec<CfgBlock<S, BlockPyTerm<E>>> {
    blocks
        .into_iter()
        .map(|block| CfgBlock {
            label: block.label.clone(),
            body: block.body,
            term: block.term,
            params: block.params,
            exc_edge: exception_edges
                .get(&block.label)
                .cloned()
                .flatten()
                .map(BlockPyEdge::new),
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn lower_structured_semantic_blocks_to_bb_blocks(
    blocks: &[CfgBlock<StructuredBlockPyStmt, BlockPyTerm>],
) -> Vec<CfgBlock<BlockPyStmt<RuffExpr, ast::ExprName>, BlockPyTerm<RuffExpr>>> {
    let exception_edges = lowered_exception_edges(blocks);
    let (linear_blocks, _linear_block_params, linear_exception_edges) =
        linearize_structured_ifs(blocks, &HashMap::new(), &exception_edges);
    let mut bb_blocks = linear_blocks
        .iter()
        .map(|block| {
            let exc_edge = linear_exception_edges
                .get(&block.label)
                .cloned()
                .flatten()
                .map(BlockPyEdge::new);
            let ops = block
                .body
                .clone()
                .into_iter()
                .map(BlockPyStmt::from)
                .collect::<Vec<_>>();
            crate::block_py::CfgBlock {
                label: block.label.clone(),
                body: ops,
                term: convert_blockpy_term_expr(block.term.clone()),
                params: block.bb_params().cloned().collect(),
                exc_edge,
            }
        })
        .collect::<Vec<_>>();
    populate_exception_edge_args(&mut bb_blocks);
    bb_blocks
}

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
        RegionTargets::new(end_label.clone(), None),
        &mut blocks,
        &name_gen,
    );
    move_entry_block_to_front(&mut blocks, entry_label.clone());
    for block in &blocks {
        assert_blockpy_block_normalized(block);
    }
    let mut structured_callable_def = BlockPyFunction::<StructuredRuffBlockPyPass> {
        function_id,
        name_gen,
        names,
        kind: blockpy_kind,
        params,
        blocks,
        doc,
        storage_layout: None,
        semantic: semantic.clone(),
    };
    let needs_end_block = entry_label == end_label
        || structured_callable_def
            .blocks
            .iter()
            .any(|block| block_references_label(block, &end_label));
    if needs_end_block {
        structured_callable_def.blocks.push(CfgBlock {
            label: end_label,
            body: Vec::new(),
            term: BlockPyTerm::implicit_function_return(),
            params: Vec::new(),
            exc_edge: None,
        });
    }
    fold_jumps_to_trivial_none_return_blockpy(&mut structured_callable_def.blocks);
    fold_constant_brif_blockpy(&mut structured_callable_def.blocks);
    let extra_roots = structured_callable_def
        .blocks
        .iter()
        .filter_map(|block| block.exc_edge.as_ref().map(|edge| edge.target.clone()))
        .collect::<Vec<_>>();
    prune_unreachable_blockpy_blocks(
        entry_label,
        &extra_roots,
        &mut structured_callable_def.blocks,
    );
    if matches!(structured_callable_def.kind, BlockPyFunctionKind::Function) {
        rewrite_current_exception_placeholders_in_lowered_blocks(
            &mut structured_callable_def.blocks,
        );
    }
    let blocks = lower_structured_semantic_blocks_to_bb_blocks(&structured_callable_def.blocks);
    BlockPyFunction {
        function_id: structured_callable_def.function_id,
        name_gen: structured_callable_def.name_gen,
        names: structured_callable_def.names,
        kind: structured_callable_def.kind,
        params: structured_callable_def.params,
        blocks,
        doc: structured_callable_def.doc,
        storage_layout: None,
        semantic: structured_callable_def.semantic,
    }
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
    blocks: &mut [crate::block_py::CfgBlock<StructuredBlockPyStmt, BlockPyTerm>],
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

fn rewrite_current_exception_placeholders_in_stmt(
    stmt: &mut StructuredBlockPyStmt,
    exc_name: &str,
) {
    match stmt {
        StructuredBlockPyStmt::Assign(assign) => {
            rewrite_current_exception_placeholders_in_expr(&mut assign.value, exc_name);
        }
        StructuredBlockPyStmt::Expr(expr) => {
            rewrite_current_exception_placeholders_in_expr(expr, exc_name);
        }
        StructuredBlockPyStmt::Delete(_) => {}
        StructuredBlockPyStmt::If(if_stmt) => {
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
    pub break_label: BlockPyLabel,
    pub continue_label: BlockPyLabel,
}

#[derive(Clone)]
pub(crate) struct RegionTargets {
    pub normal_cont: BlockPyLabel,
    pub loop_labels: Option<LoopLabels>,
    pub active_exc: Option<BlockPyLabel>,
}

impl RegionTargets {
    pub(crate) fn new(
        normal_cont: impl Into<BlockPyLabel>,
        active_exc: Option<BlockPyLabel>,
    ) -> Self {
        Self {
            normal_cont: normal_cont.into(),
            loop_labels: None,
            active_exc,
        }
    }

    pub(crate) fn nested(&self, normal_cont: impl Into<BlockPyLabel>) -> Self {
        Self {
            normal_cont: normal_cont.into(),
            loop_labels: self.loop_labels.clone(),
            active_exc: self.active_exc.clone(),
        }
    }

    pub(crate) fn nested_with_loop(
        &self,
        normal_cont: impl Into<BlockPyLabel>,
        loop_labels: Option<LoopLabels>,
    ) -> Self {
        Self {
            normal_cont: normal_cont.into(),
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
