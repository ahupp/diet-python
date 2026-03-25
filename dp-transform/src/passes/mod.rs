pub(crate) mod ast_symbol_analysis;
pub(crate) mod ast_to_ast;
pub(crate) mod blockpy_expr_simplify;
mod blockpy_generators;
pub mod blockpy_to_bb;
pub(crate) mod core_await_lower;
pub(crate) mod core_eval_order;
mod name_binding;
pub mod ruff_to_blockpy;
mod summarize_pass_shape;
mod trace;

use crate::block_py::{
    BlockPyPass, BlockPyStmt, CoreBlockPyExpr, CoreBlockPyExprWithAwaitAndYield,
    CoreBlockPyExprWithYield, Expr,
};

#[derive(Debug, Clone)]
pub struct RuffBlockPyPass;

impl BlockPyPass for RuffBlockPyPass {
    type Expr = Expr;
    type Stmt = BlockPyStmt<Self::Expr>;
}

#[derive(Debug, Clone)]
pub struct CoreBlockPyPassWithAwaitAndYield;

impl BlockPyPass for CoreBlockPyPassWithAwaitAndYield {
    type Expr = CoreBlockPyExprWithAwaitAndYield;
    type Stmt = BlockPyStmt<Self::Expr>;
}

#[derive(Debug, Clone)]
pub struct CoreBlockPyPassWithYield;

impl BlockPyPass for CoreBlockPyPassWithYield {
    type Expr = CoreBlockPyExprWithYield;
    type Stmt = BlockPyStmt<Self::Expr>;
}

#[derive(Debug, Clone)]
pub struct CoreBlockPyPass;

impl BlockPyPass for CoreBlockPyPass {
    type Expr = CoreBlockPyExpr;
    type Stmt = BlockPyStmt<Self::Expr>;
}

#[derive(Debug, Clone)]
pub struct BbBlockPyPass;

impl BlockPyPass for BbBlockPyPass {
    type Expr = CoreBlockPyExpr;
    type Stmt = crate::block_py::BbStmt;
}

#[derive(Debug, Clone)]
pub struct PreparedBbBlockPyPass;

impl BlockPyPass for PreparedBbBlockPyPass {
    type Expr = CoreBlockPyExpr;
    type Stmt = crate::block_py::BbStmt;
}

pub(crate) use blockpy_to_bb::{
    lower_core_blockpy_module_bundle_to_bb_module,
    lower_yield_in_lowered_core_blockpy_module_bundle,
};
pub use blockpy_to_bb::{lower_try_jump_exception_flow, normalize_bb_module_for_codegen};

pub(crate) use name_binding::lower_name_binding_in_core_blockpy_module;
pub(crate) use summarize_pass_shape::summarize_tracked_pass_shape;

#[cfg(test)]
mod test;
