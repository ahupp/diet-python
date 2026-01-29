class Example:
    value = __name__


RESULT = Example.value

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
assert module.RESULT == module.__name__
