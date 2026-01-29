from collections import namedtuple


class Base(namedtuple("Base", "a b")):
    def __new__(cls, value):
        return super().__new__(cls, value, value)


class Child(Base):
    pass


def build_child():
    return Child(1)

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
instance = module.build_child()
assert instance.a == 1
assert instance.b == 1
