from enum import Enum


def build_enum():
    class Example(Enum):
        VALUE = 1

        def __new__(cls, value):
            obj = super().__new__(cls, value)
            obj._value_ = value
            return obj

    return Example
