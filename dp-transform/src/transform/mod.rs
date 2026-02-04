pub(crate) mod ast_rewrite;
pub(crate) mod context;
pub(crate) mod driver;
pub(crate) mod rewrite_class_def;
pub(crate) mod rewrite_expr;
pub(crate) mod rewrite_function_def;
pub(crate) mod rewrite_future_annotations;
pub(crate) mod rewrite_import;
pub(crate) mod rewrite_names;
pub(crate) mod rewrite_stmt;
pub(crate) mod scope;
pub(crate) mod simplify;
pub(crate) mod util;

#[derive(Clone, Copy)]
pub struct Options {
    pub inject_import: bool,
    pub cpython: bool,
    pub eval_mode: bool,
    pub lower_attributes: bool,
    pub truthy: bool,
    pub cleanup_dp_globals: bool,
    pub force_import_rewrite: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            inject_import: true,
            lower_attributes: false,
            cpython: false,
            eval_mode: false,
            truthy: false,
            cleanup_dp_globals: true,
            force_import_rewrite: false,
        }
    }
}

impl Options {
    pub fn for_test() -> Self {
        Self {
            inject_import: false,
            lower_attributes: false,
            cpython: false,
            eval_mode: false,
            truthy: false,
            cleanup_dp_globals: false,
            force_import_rewrite: false,
        }
    }
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!(snapshot_expr_fixture, "snapshot_expr.txt");
    crate::transform_fixture_test!(snapshot_stmt_fixture, "snapshot_stmt.txt");
    crate::transform_fixture_test!(snapshot_class_fixture, "snapshot_class.txt");
}
