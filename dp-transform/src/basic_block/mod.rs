mod annotation_export;
mod ast_symbol_analysis;
pub(crate) mod ast_to_ast;
mod await_lower;
pub mod bb_ir;
pub mod block_py;
mod blockpy_expr_simplify;
mod blockpy_generators;
mod blockpy_to_bb;
mod bound_names;
pub(crate) mod cfg_ir;
mod expr_utils;
mod function_identity;
mod function_lowering;
mod param_specs;
mod ruff_to_blockpy;
mod stmt_utils;

#[cfg(test)]
mod driver;

// Ruff AST -> BbModule
pub use block_py::pretty::blockpy_module_to_string;
pub(crate) use blockpy_to_bb::{
    lower_try_jump_exception_flow, lowered_blockpy_module_bundle_to_blockpy_module,
    normalize_bb_module_for_codegen,
};
pub use function_identity::FunctionIdentityByNode;
pub use function_lowering::BBSimplifyStmtPass;

use self::ast_to_ast::rewrite_stmt::function_def::rewrite_ast_to_lowered_blockpy_module;
use self::blockpy_to_bb::lower_blockpy_module_bundle_to_bb_module;
use self::function_identity::{collect_function_identity_private, FunctionIdentity};
use ast_to_ast::context::Context;
use ast_to_ast::scope::Scope;
use bb_ir::BbModule;
use block_py::BlockPyModule;
use ruff_python_ast::StmtBody;
use std::sync::Arc;

pub fn collect_function_identity_by_node(
    module: &mut StmtBody,
    module_scope: Arc<Scope>,
) -> FunctionIdentityByNode {
    collect_function_identity_private(module, module_scope)
        .into_iter()
        .map(|(node, identity)| {
            (
                node,
                (
                    identity.bind_name,
                    identity.display_name,
                    identity.qualname,
                    identity.binding_target,
                ),
            )
        })
        .collect()
}

pub fn rewrite_with_function_identity_and_collect_ir(
    context: &Context,
    module: &mut StmtBody,
    function_identity_by_node: FunctionIdentityByNode,
) -> BbModule {
    rewrite_internal(context, module, Some(function_identity_by_node))
}

pub fn rewrite_with_function_identity_to_blockpy_module(
    context: &Context,
    module: &mut StmtBody,
    function_identity_by_node: FunctionIdentityByNode,
) -> BlockPyModule {
    let lowered_module =
        rewrite_ast_to_lowered_blockpy_module(context, module, function_identity_by_node);
    lowered_blockpy_module_bundle_to_blockpy_module(&lowered_module)
}

fn rewrite_internal(
    context: &Context,
    module: &mut StmtBody,
    function_identity_by_node: Option<FunctionIdentityByNode>,
) -> BbModule {
    let function_identity_by_node = function_identity_by_node.unwrap_or_else(|| {
        let module_scope = ast_to_ast::scope::analyze_module_scope(module);
        collect_function_identity_private(module, module_scope)
            .into_iter()
            .map(
                |(
                    node,
                    FunctionIdentity {
                        bind_name,
                        display_name,
                        qualname,
                        binding_target,
                    },
                )| { (node, (bind_name, display_name, qualname, binding_target)) },
            )
            .collect()
    });
    let lowered_module =
        rewrite_ast_to_lowered_blockpy_module(context, module, function_identity_by_node);
    lower_blockpy_module_bundle_to_bb_module(context, &lowered_module)
}

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
