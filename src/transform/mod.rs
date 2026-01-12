pub(crate) mod class_def;
pub(crate) mod context;
pub(crate) mod driver;
pub(crate) mod rewrite_assign_del;
pub(crate) mod rewrite_decorator;
pub(crate) mod rewrite_expr_to_stmt;
pub(crate) mod rewrite_func_expr;
pub(crate) mod rewrite_future_annotations;
pub(crate) mod rewrite_import;
pub(crate) mod rewrite_match_case;
pub(crate) mod simple;

#[derive(Clone, Copy)]
pub enum ImportStarHandling {
    Allowed,
    Error,
    Strip,
}

#[derive(Clone, Copy)]
pub struct Options {
    pub import_star_handling: ImportStarHandling,
    pub inject_import: bool,
    pub lower_attributes: bool,
    pub truthy: bool,
    pub cleanup_dp_globals: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            import_star_handling: ImportStarHandling::Allowed,
            inject_import: true,
            lower_attributes: true,
            truthy: false,
            cleanup_dp_globals: true,
        }
    }
}

impl Options {
    pub fn for_test() -> Self {
        Self {
            import_star_handling: ImportStarHandling::Error,
            inject_import: false,
            lower_attributes: false,
            truthy: false,
            cleanup_dp_globals: false,
        }
    }
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!(class_scope_fixture, "test_class_scope.txt");
    crate::transform_fixture_test!("tests_mod.txt");
}
