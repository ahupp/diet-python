pub(crate) mod context;
pub(crate) mod driver;
pub(crate) mod rewrite_assert;
pub(crate) mod rewrite_assign_del;
pub(crate) mod rewrite_class_def;
pub(crate) mod rewrite_decorator;
pub(crate) mod rewrite_exception;
pub(crate) mod rewrite_expr_to_stmt;
pub(crate) mod rewrite_func_expr;
pub(crate) mod rewrite_future_annotations;
pub(crate) mod rewrite_import;
pub(crate) mod rewrite_loop;
pub(crate) mod rewrite_match_case;
pub(crate) mod rewrite_string;
pub(crate) mod rewrite_truthy;
pub(crate) mod rewrite_with;

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
            import_star_handling: ImportStarHandling::Allowed,
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

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!("tests_mod.txt");
}
