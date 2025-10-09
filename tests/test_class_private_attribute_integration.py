import pytest


@pytest.mark.integration
def test_private_attribute_uses_name_mangling(run_integration_module):
    with run_integration_module("class_private_attribute") as module:
        Example = module.Example

        instance = Example("initial")
        assert instance._Example__value == "initial"

        instance.update("payload")
        assert instance._Example__value == "payload"
        assert module.use_example() == "payload"

        with pytest.raises(AttributeError, match="__value"):
            getattr(instance, "__value")
