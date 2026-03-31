def test_class_private_attribute_mangling(run_integration_module):
    with run_integration_module("class_private_attribute") as module:
        assert module.use_example() == "payload"


def test_class_private_attribute_setattr(run_integration_module):
    with run_integration_module("class_private_attribute_set") as module:
        assert module.run() == "ok"
