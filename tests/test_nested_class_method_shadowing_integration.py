def test_nested_class_method_name_does_not_shadow_outer_method(run_integration_module):
    with run_integration_module("nested_class_method_shadowing") as module:
        assert isinstance(module.VALUE, float)
