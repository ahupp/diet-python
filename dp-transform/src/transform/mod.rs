pub(crate) mod class_def;
pub(crate) mod context;
pub(crate) mod driver;
pub(crate) mod rewrite_assign_del;
pub(crate) mod rewrite_decorator;
pub(crate) mod rewrite_expr_to_stmt;
pub(crate) mod rewrite_explicit_scope;
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
    pub force_import_rewrite: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            import_star_handling: ImportStarHandling::Allowed,
            inject_import: true,
            lower_attributes: true,
            truthy: false,
            cleanup_dp_globals: true,
            force_import_rewrite: false,
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
            force_import_rewrite: false,
        }
    }
}

#[cfg(test)]
mod tests {
    crate::transform_fixture_test!(class_scope_fixture, "test_class_scope.txt");
    crate::transform_fixture_test!(
        multiprocessing_barrier_fixture,
        "test_multiprocessing_barrier_abort_reset.txt"
    );
    crate::transform_fixture_test!(cleanup_dp_globals_fixture, "test_cleanup_dp_globals.txt");
    crate::transform_fixture_test!(
        named_expression_cases_fixture,
        "test_named_expression_cases.txt"
    );
    crate::transform_fixture_test!(
        typing_generics_cases_fixture,
        "test_typing_generics_cases.txt"
    );
    crate::transform_fixture_test!(
        pep695_type_aliases_fixture,
        "test_pep695_type_aliases.txt"
    );
    crate::transform_fixture_test!(
        generator_exception_context_fixture,
        "test_generator_exception_context.txt"
    );
    crate::transform_fixture_test!(
        with_extended_targets_fixture,
        "test_with_extended_targets.txt"
    );
    crate::transform_fixture_test!(listcomp_classcell_fixture, "test_listcomp_classcell.txt");
    crate::transform_fixture_test!(
        asyncio_taskgroup_base_error_refcycle_fixture,
        "test_asyncio_taskgroup_base_error_refcycle.txt"
    );
    crate::transform_fixture_test!(
        dataclasses_make_dataclass_invalid_field_fixture,
        "test_dataclasses_make_dataclass_invalid_field.txt"
    );
    crate::transform_fixture_test!(
        class_annotations_deferred_fixture,
        "test_class_annotations_deferred.txt"
    );
    crate::transform_fixture_test!("tests_mod.txt");
}
