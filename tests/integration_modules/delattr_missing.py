"""Ensure the transform rewrites `del` to `__dp__.delattr` correctly."""


class Example:
    pass


INSTANCE = Example()
INSTANCE.value = 1
del INSTANCE.value
ATTRIBUTE_DELETED = not hasattr(INSTANCE, "value")

# diet-python: validate

def validate_module(module):

    import pytest

    assert module.ATTRIBUTE_DELETED is True
