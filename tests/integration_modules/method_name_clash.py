class date:
    __slots__ = ()


class Example:
    slots = date.__slots__

    def date(self):
        return date()

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
instance = module.Example()
result = instance.date()
assert isinstance(result, module.date)
