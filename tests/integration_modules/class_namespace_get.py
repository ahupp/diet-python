import types


class Example:
    def get(self):
        return "value"

    __class_getitem__ = classmethod(types.GenericAlias)


RESULT = Example[int]

# diet-python: validate

from __future__ import annotations

def validate(module):
    assert module.RESULT == module.Example[int]
