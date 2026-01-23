class Example:
    def __init__(self, value):
        self.__value = value

    def update(self, value):
        self.__value = value

    def read(self):
        return self.__value


def use_example():
    instance = Example("initial")
    instance.update("payload")
    return instance.read()

# diet-python: validate

import pytest

def validate(module):
    Example = module.Example

    instance = Example("initial")
    assert instance._Example__value == "initial"

    instance.update("payload")
    assert instance._Example__value == "payload"
    assert module.use_example() == "payload"

    with pytest.raises(AttributeError, match="__value"):
        getattr(instance, "__value")
