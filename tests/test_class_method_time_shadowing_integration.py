def test_class_method_name_does_not_shadow_module_time(run_integration_module):
    with run_integration_module("class_method_time_shadowing") as module:
        assert isinstance(module.VALUE, float)
