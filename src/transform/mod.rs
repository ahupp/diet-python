pub(crate) mod context;
pub(crate) mod expr;
pub(crate) mod rewrite_assert;
pub(crate) mod rewrite_class_def;
pub(crate) mod rewrite_complex_expr;
pub(crate) mod rewrite_decorator;
pub(crate) mod rewrite_expr_to_stmt;
pub(crate) mod rewrite_for_loop;
pub(crate) mod rewrite_import;
pub(crate) mod rewrite_match_case;
pub(crate) mod rewrite_string;
pub(crate) mod rewrite_try_except;
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

#[cfg(test)]
mod tests {
    use super::Options;
    use crate::test_util::assert_transform_eq;
    use crate::{ruff_ast_to_string, transform_str_to_ruff_with_options};

    #[test]
    fn strips_type_alias_statement() {
        assert_transform_eq("type Alias = int", "");
    }

    #[test]
    fn strips_type_aliases_in_if_branches() {
        let input = r#"
if True:
    type Alias = int
    x = 1
elif False:
    type Alias = str
    y = 2
else:
    type Alias = bytes
    z = 3
"#;
        let expected = r#"
if True:
    x = 1
elif False:
    y = 2
else:
    z = 3
"#;
        assert_transform_eq(input, expected);
    }

    #[test]
    fn strips_type_alias_from_class_body() {
        let input = r#"
type Alias = int

class Foo:
    type Inner = str

    def method(self):
        return 1
"#;
        let alias_free = r#"
type Alias = int

class Foo:
    def method(self):
        return 1
"#;
        let module = transform_str_to_ruff_with_options(alias_free, Options::for_test()).unwrap();
        let expected = ruff_ast_to_string(&module.body);
        assert_transform_eq(input, expected.as_str());
    }
}
