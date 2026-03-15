use super::ast_to_ast::rewrite_expr::string::{
    lower_string_templates_in_expr, lower_string_templates_in_parameters,
};
use super::block_py::{
    BlockPyExpr, BlockPyIf, BlockPyIfTerm, BlockPyRaise, BlockPyStmt, BlockPyTerm,
    SemanticBlockPyCallableDef, SemanticBlockPyStmtFragment,
};
use super::blockpy_to_bb::{LoweredBlockPyModuleBundle, LoweredCallableDef};
use super::ruff_to_blockpy::LoweredBlockPyFunction;

fn lower_string_templates_in_blockpy_expr(expr: &mut BlockPyExpr) {
    expr.rewrite_mut(lower_string_templates_in_expr);
}

fn lower_string_templates_in_blockpy_term(term: &mut BlockPyTerm) {
    match term {
        BlockPyTerm::Jump(_) | BlockPyTerm::TryJump(_) => {}
        BlockPyTerm::IfTerm(BlockPyIfTerm { test, .. }) => {
            lower_string_templates_in_blockpy_expr(test);
        }
        BlockPyTerm::BranchTable(branch_table) => {
            lower_string_templates_in_blockpy_expr(&mut branch_table.index);
        }
        BlockPyTerm::Raise(BlockPyRaise { exc }) => {
            if let Some(exc) = exc {
                lower_string_templates_in_blockpy_expr(exc);
            }
        }
        BlockPyTerm::Return(value) => {
            if let Some(value) = value {
                lower_string_templates_in_blockpy_expr(value);
            }
        }
    }
}

fn lower_string_templates_in_blockpy_fragment(fragment: &mut SemanticBlockPyStmtFragment) {
    for stmt in &mut fragment.body {
        match stmt {
            BlockPyStmt::Assign(assign) => {
                lower_string_templates_in_blockpy_expr(&mut assign.value);
            }
            BlockPyStmt::Expr(expr) => lower_string_templates_in_blockpy_expr(expr),
            BlockPyStmt::Delete(_) => {}
            BlockPyStmt::If(BlockPyIf { test, body, orelse }) => {
                lower_string_templates_in_blockpy_expr(test);
                lower_string_templates_in_blockpy_fragment(body);
                lower_string_templates_in_blockpy_fragment(orelse);
            }
        }
    }
    if let Some(term) = &mut fragment.term {
        lower_string_templates_in_blockpy_term(term);
    }
}

fn lower_string_templates_in_callable_def(
    callable_def: &SemanticBlockPyCallableDef,
) -> SemanticBlockPyCallableDef {
    let mut callable_def = callable_def.clone();
    lower_string_templates_in_parameters(&mut callable_def.params);
    if let Some(doc) = &mut callable_def.doc {
        lower_string_templates_in_blockpy_expr(doc);
    }
    for block in &mut callable_def.blocks {
        for stmt in &mut block.body {
            match stmt {
                BlockPyStmt::Assign(assign) => {
                    lower_string_templates_in_blockpy_expr(&mut assign.value);
                }
                BlockPyStmt::Expr(expr) => lower_string_templates_in_blockpy_expr(expr),
                BlockPyStmt::Delete(_) => {}
                BlockPyStmt::If(BlockPyIf { test, body, orelse }) => {
                    lower_string_templates_in_blockpy_expr(test);
                    lower_string_templates_in_blockpy_fragment(body);
                    lower_string_templates_in_blockpy_fragment(orelse);
                }
            }
        }
        lower_string_templates_in_blockpy_term(&mut block.term);
    }
    callable_def
}

fn lower_string_templates_in_lowered_blockpy_function(
    lowered: &LoweredBlockPyFunction,
) -> LoweredBlockPyFunction {
    LoweredBlockPyFunction {
        callable_def: lower_string_templates_in_callable_def(&lowered.callable_def),
        is_coroutine: lowered.is_coroutine,
        bb_kind: lowered.bb_kind.clone(),
        block_params: lowered.block_params.clone(),
        exception_edges: lowered.exception_edges.clone(),
        closure_layout: lowered.closure_layout.clone(),
        param_specs: lowered.param_specs.clone(),
    }
}

pub(crate) fn lower_string_templates_in_lowered_blockpy_module_bundle(
    module: &LoweredBlockPyModuleBundle,
) -> LoweredBlockPyModuleBundle {
    module.map_callable_defs(|lowered_function| LoweredCallableDef {
        callable_def: lower_string_templates_in_lowered_blockpy_function(
            &lowered_function.callable_def,
        ),
        binding_target: lowered_function.binding_target,
    })
}
