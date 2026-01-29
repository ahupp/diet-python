class Example:
    def __init__(self) -> None:
        self._value = 0

    @property
    def value(self) -> int:
        return self._value

    @value.setter
    def value(self, value: int) -> None:
        self._value = value

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
"""Property setters should round-trip values under the transform."""
instance = module.Example()
instance.value = 5
assert instance.value == 5
