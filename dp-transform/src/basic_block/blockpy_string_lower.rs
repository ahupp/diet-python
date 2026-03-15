use super::ast_to_ast::context::Context;
use super::ast_to_ast::rewrite_expr::string::{
    lower_string_templates_in_expr, lower_string_templates_in_parameters,
};
use super::block_py::{
    BlockPyExpr, BlockPyIf, BlockPyIfTerm, BlockPyRaise, BlockPyStmt, BlockPyTerm,
    SemanticBlockPyCallableDef, SemanticBlockPyStmtFragment,
};
use super::blockpy_to_bb::{LoweredBlockPyModuleBundle, LoweredCallableDef};
use super::ruff_to_blockpy::LoweredBlockPyFunction;

fn lower_string_templates_in_blockpy_expr(context: &Context, expr: &mut BlockPyExpr) {
    expr.rewrite_mut(|expr| lower_string_templates_in_expr(context, expr));
}

fn lower_string_templates_in_blockpy_term(context: &Context, term: &mut BlockPyTerm) {
    match term {
        BlockPyTerm::Jump(_) | BlockPyTerm::TryJump(_) => {}
        BlockPyTerm::IfTerm(BlockPyIfTerm { test, .. }) => {
            lower_string_templates_in_blockpy_expr(context, test);
        }
        BlockPyTerm::BranchTable(branch_table) => {
            lower_string_templates_in_blockpy_expr(context, &mut branch_table.index);
        }
        BlockPyTerm::Raise(BlockPyRaise { exc }) => {
            if let Some(exc) = exc {
                lower_string_templates_in_blockpy_expr(context, exc);
            }
        }
        BlockPyTerm::Return(value) => {
            if let Some(value) = value {
                lower_string_templates_in_blockpy_expr(context, value);
            }
        }
    }
}

fn lower_string_templates_in_blockpy_fragment(
    context: &Context,
    fragment: &mut SemanticBlockPyStmtFragment,
) {
    for stmt in &mut fragment.body {
        match stmt {
            BlockPyStmt::Assign(assign) => {
                lower_string_templates_in_blockpy_expr(context, &mut assign.value);
            }
            BlockPyStmt::Expr(expr) => lower_string_templates_in_blockpy_expr(context, expr),
            BlockPyStmt::Delete(_) => {}
            BlockPyStmt::If(BlockPyIf { test, body, orelse }) => {
                lower_string_templates_in_blockpy_expr(context, test);
                lower_string_templates_in_blockpy_fragment(context, body);
                lower_string_templates_in_blockpy_fragment(context, orelse);
            }
        }
    }
    if let Some(term) = &mut fragment.term {
        lower_string_templates_in_blockpy_term(context, term);
    }
}

fn lower_string_templates_in_callable_def(
    context: &Context,
    callable_def: &SemanticBlockPyCallableDef,
) -> SemanticBlockPyCallableDef {
    let mut callable_def = callable_def.clone();
    lower_string_templates_in_parameters(context, &mut callable_def.params);
    if let Some(doc) = &mut callable_def.doc {
        lower_string_templates_in_blockpy_expr(context, doc);
    }
    for block in &mut callable_def.blocks {
        for stmt in &mut block.body {
            match stmt {
                BlockPyStmt::Assign(assign) => {
                    lower_string_templates_in_blockpy_expr(context, &mut assign.value);
                }
                BlockPyStmt::Expr(expr) => lower_string_templates_in_blockpy_expr(context, expr),
                BlockPyStmt::Delete(_) => {}
                BlockPyStmt::If(BlockPyIf { test, body, orelse }) => {
                    lower_string_templates_in_blockpy_expr(context, test);
                    lower_string_templates_in_blockpy_fragment(context, body);
                    lower_string_templates_in_blockpy_fragment(context, orelse);
                }
            }
        }
        lower_string_templates_in_blockpy_term(context, &mut block.term);
    }
    callable_def
}

fn lower_string_templates_in_lowered_blockpy_function(
    context: &Context,
    lowered: &LoweredBlockPyFunction,
) -> LoweredBlockPyFunction {
    LoweredBlockPyFunction {
        callable_def: lower_string_templates_in_callable_def(context, &lowered.callable_def),
        is_coroutine: lowered.is_coroutine,
        bb_kind: lowered.bb_kind.clone(),
        block_params: lowered.block_params.clone(),
        exception_edges: lowered.exception_edges.clone(),
        closure_layout: lowered.closure_layout.clone(),
        param_specs: lowered.param_specs.clone(),
    }
}

pub(crate) fn lower_string_templates_in_lowered_blockpy_module_bundle(
    context: &Context,
    module: &LoweredBlockPyModuleBundle,
) -> LoweredBlockPyModuleBundle {
    module.map_callable_defs(|lowered_function| LoweredCallableDef {
        callable_def: lower_string_templates_in_lowered_blockpy_function(
            context,
            &lowered_function.callable_def,
        ),
        binding_target: lowered_function.binding_target,
    })
}
