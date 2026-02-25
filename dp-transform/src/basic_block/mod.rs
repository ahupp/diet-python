mod ast_to_bb;
pub mod bb_ir;
mod codegen_normalize;
mod render_py;

pub use ast_to_bb::{
    collect_function_identity_by_node, rewrite_with_function_identity_and_collect_ir,
    BBSimplifyStmtPass,
};
pub use codegen_normalize::normalize_bb_module_for_codegen;
