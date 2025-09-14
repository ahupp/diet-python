pub(crate) mod expr;
pub(crate) mod rewrite_assert;
pub(crate) mod rewrite_class_def;
pub(crate) mod rewrite_decorator;
pub(crate) mod rewrite_for_loop;
pub(crate) mod rewrite_import;
pub(crate) mod rewrite_match_case;
pub(crate) mod rewrite_try_except;
pub(crate) mod rewrite_with;
pub(crate) mod rewrite_string;
pub(crate) mod truthy;

#[derive(Clone, Copy)]
pub struct Options {
    pub allow_import_star: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            allow_import_star: true,
        }
    }
}
