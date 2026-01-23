def test_class_scope_inner_capture_integration(run_integration_module):
    with run_integration_module("class_scope_inner_capture") as module:
        assert module.RESULT == "outer"
