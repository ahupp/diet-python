class Example:
    value = 1
    del value


EXPECTS_VALUE = hasattr(Example, "value")

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
assert module.EXPECTS_VALUE is False
