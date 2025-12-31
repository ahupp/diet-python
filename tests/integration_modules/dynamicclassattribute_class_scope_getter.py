from __future__ import annotations

from types import DynamicClassAttribute


class Base:
    @DynamicClassAttribute
    def spam(self):
        return 1


class Sub(Base):
    spam = Base.__dict__["spam"]

    @spam.getter
    def spam(self):
        return 2


def get_value():
    return Sub().spam
