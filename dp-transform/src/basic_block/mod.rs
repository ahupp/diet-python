mod ast_to_bb;
pub mod bb_ir;
mod codegen_normalize;

pub use ast_to_bb::{
    collect_function_identity_by_node, lower_try_jump_exception_flow,
    rewrite_with_function_identity_and_collect_ir, BBSimplifyStmtPass,
};
pub use codegen_normalize::normalize_bb_module_for_codegen;
