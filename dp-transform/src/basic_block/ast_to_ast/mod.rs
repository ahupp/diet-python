pub(crate) mod ast_rewrite;
pub(crate) mod context;
pub(crate) mod rewrite_class_def;
pub(crate) mod rewrite_expr;
pub(crate) mod rewrite_future_annotations;
pub(crate) mod rewrite_import;
pub(crate) mod rewrite_names;
pub(crate) mod rewrite_stmt;
pub(crate) mod scope;
pub(crate) mod simplify;
pub(crate) mod util;

#[derive(Clone, Copy)]
pub struct Options {
    pub cpython: bool,
    pub eval_mode: bool,
    pub lower_attributes: bool,
    pub force_import_rewrite: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            lower_attributes: false,
            cpython: false,
            eval_mode: false,
            force_import_rewrite: false,
        }
    }
}

impl Options {
    pub fn for_test() -> Self {
        Self {
            lower_attributes: false,
            cpython: false,
            eval_mode: false,
            force_import_rewrite: false,
        }
    }
}
