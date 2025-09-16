pub(crate) mod context;
pub(crate) mod expr;
pub(crate) mod rewrite_assert;
pub(crate) mod rewrite_class_def;
pub(crate) mod rewrite_decorator;
pub(crate) mod rewrite_expr_to_stmt;
pub(crate) mod rewrite_for_loop;
pub(crate) mod rewrite_import;
pub(crate) mod rewrite_match_case;
pub(crate) mod rewrite_string;
pub(crate) mod rewrite_try_except;
pub(crate) mod rewrite_with;
pub(crate) mod unnest;

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
}

impl Default for Options {
    fn default() -> Self {
        Self {
            import_star_handling: ImportStarHandling::Strip,
            inject_import: true,
            lower_attributes: true,
            truthy: false,
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
        }
    }
}
