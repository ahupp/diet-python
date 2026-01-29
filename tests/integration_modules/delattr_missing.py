"""Ensure the transform rewrites `del` to `__dp__.delattr` correctly."""


class Example:
    pass


INSTANCE = Example()
INSTANCE.value = 1
del INSTANCE.value
ATTRIBUTE_DELETED = not hasattr(INSTANCE, "value")

# diet-python: validate

from __future__ import annotations

import pytest

module = __import__("sys").modules[__name__]
assert module.ATTRIBUTE_DELETED is True
