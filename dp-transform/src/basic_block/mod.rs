mod ast_to_bb;
mod bb_passes;
pub mod bb_ir;

// Ruff AST -> BbModule
pub use ast_to_bb::{
    collect_function_identity_by_node, rewrite_with_function_identity_and_collect_ir,
    BBSimplifyStmtPass,
};
// BbModule -> BbModule
pub use bb_passes::{lower_try_jump_exception_flow, normalize_bb_module_for_codegen};

pub fn rewrite_ast_to_bb_module(
    context: &crate::transform::context::Context,
    module: &mut ruff_python_ast::StmtBody,
    function_identity_by_node: ast_to_bb::FunctionIdentityByNode,
) -> bb_ir::BbModule {
    rewrite_with_function_identity_and_collect_ir(context, module, function_identity_by_node)
}

pub fn prepare_bb_module_for_jit(module: &bb_ir::BbModule) -> Result<bb_ir::BbModule, String> {
    lower_try_jump_exception_flow(module)
}

pub fn prepare_bb_module_for_codegen(module: &bb_ir::BbModule) -> bb_ir::BbModule {
    normalize_bb_module_for_codegen(module)
}
