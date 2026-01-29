from enum import Enum


def build_enum():
    class Example(Enum):
        VALUE = 1

        def __new__(cls, value):
            obj = super().__new__(cls, value)
            obj._value_ = value
            return obj

    return Example

# diet-python: validate

from __future__ import annotations

import pytest

module = __import__("sys").modules[__name__]
with pytest.raises(TypeError, match="do not use `super\\(\\).__new__"):
    module.build_enum()
