def test_super_uses_defining_class(run_integration_module):
    with run_integration_module("super_rebind_class_name") as module:
        assert module.VALUE == "base"
        assert module.INSTANCE.child is True
