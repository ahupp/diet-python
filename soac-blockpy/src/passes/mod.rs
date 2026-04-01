pub(crate) mod ast_symbol_analysis;
pub(crate) mod ast_to_ast;
pub(crate) mod blockpy_expr_simplify;
mod blockpy_generators;
pub mod blockpy_to_bb;
pub(crate) mod core_await_lower;
pub(crate) mod core_eval_order;
mod name_binding;
pub mod ruff_to_blockpy;
mod trace;

use crate::block_py::{cfg::relabel_blockpy_blocks_dense, BlockPyModule};
use crate::block_py::{
    BlockPyPass, BlockPyStmt, CodegenBlockPyExpr, CoreBlockPyExpr,
    CoreBlockPyExprWithAwaitAndYield, CoreBlockPyExprWithYield, LocatedName,
};
use ruff_python_ast as ast;

#[derive(Debug, Clone)]
pub struct CoreBlockPyPassWithAwaitAndYield;

impl BlockPyPass for CoreBlockPyPassWithAwaitAndYield {
    type Expr = CoreBlockPyExprWithAwaitAndYield;
    type Stmt = BlockPyStmt<Self::Expr, ast::ExprName>;
}

#[derive(Debug, Clone)]
pub struct CoreBlockPyPassWithYield;

impl BlockPyPass for CoreBlockPyPassWithYield {
    type Expr = CoreBlockPyExprWithYield;
    type Stmt = BlockPyStmt<Self::Expr, ast::ExprName>;
}

#[derive(Debug, Clone)]
pub struct CoreBlockPyPass;

impl BlockPyPass for CoreBlockPyPass {
    type Expr = CoreBlockPyExpr<ast::ExprName>;
    type Stmt = BlockPyStmt<Self::Expr, ast::ExprName>;
}

#[derive(Debug, Clone)]
pub struct ResolvedStorageBlockPyPass;

impl BlockPyPass for ResolvedStorageBlockPyPass {
    type Expr = CoreBlockPyExpr<LocatedName>;
    type Stmt = BlockPyStmt<Self::Expr, LocatedName>;
}

#[derive(Debug, Clone)]
pub struct CodegenBlockPyPass;

impl BlockPyPass for CodegenBlockPyPass {
    type Expr = CodegenBlockPyExpr;
    type Stmt = BlockPyStmt<Self::Expr, LocatedName>;
}

pub(crate) use blockpy_to_bb::lower_yield_in_lowered_core_blockpy_module_bundle;
pub use blockpy_to_bb::{lower_try_jump_exception_flow, normalize_bb_module_strings};

pub(crate) use name_binding::lower_name_binding_in_core_blockpy_module;
pub(crate) use trace::{instrument_bb_module_for_trace, parse_trace_env};

pub fn relabel_dense_bb_module(module: &mut BlockPyModule<CodegenBlockPyPass>) {
    for callable in &mut module.callable_defs {
        relabel_blockpy_blocks_dense(&mut callable.blocks);
    }
}

#[cfg(test)]
mod test;
