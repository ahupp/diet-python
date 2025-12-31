def test_nested_class_base(run_integration_module):
    with run_integration_module("nested_class_base") as module:
        assert module.get_base_name() == "BaseThing"
