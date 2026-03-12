mod annotation_export;
mod ast_symbol_analysis;
pub(crate) mod ast_to_ast;
mod await_lower;
pub mod bb_ir;
pub mod block_py;
mod blockpy_to_bb;
mod bound_names;
mod deleted_names;
mod driver;
mod function_identity;
mod function_lowering;
mod ruff_to_blockpy;
mod stmt_utils;

// Ruff AST -> BbModule
pub use block_py::pretty::blockpy_module_to_string;
pub(crate) use blockpy_to_bb::{lower_try_jump_exception_flow, normalize_bb_module_for_codegen};
pub use driver::{
    collect_function_identity_by_node, rewrite_with_function_identity_and_collect_ir,
    rewrite_with_function_identity_to_blockpy_module,
};
pub use function_identity::FunctionIdentityByNode;
pub use function_lowering::BBSimplifyStmtPass;
pub use ruff_to_blockpy::rewrite_ast_to_blockpy_module;

pub fn rewrite_ast_to_bb_module(
    context: &crate::basic_block::ast_to_ast::context::Context,
    module: &mut ruff_python_ast::StmtBody,
    function_identity_by_node: FunctionIdentityByNode,
) -> bb_ir::BbModule {
    rewrite_with_function_identity_and_collect_ir(context, module, function_identity_by_node)
}

pub fn rewrite_ast_to_blockpy_module_with_context(
    context: &crate::basic_block::ast_to_ast::context::Context,
    module: &mut ruff_python_ast::StmtBody,
    function_identity_by_node: FunctionIdentityByNode,
) -> block_py::BlockPyModule {
    rewrite_with_function_identity_to_blockpy_module(context, module, function_identity_by_node)
}

pub fn prepare_bb_module_for_jit(module: &bb_ir::BbModule) -> Result<bb_ir::BbModule, String> {
    lower_try_jump_exception_flow(module)
}

pub fn prepare_bb_module_for_codegen(module: &bb_ir::BbModule) -> bb_ir::BbModule {
    normalize_bb_module_for_codegen(module)
}
