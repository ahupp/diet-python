from typing import TypeVar


RESULT = TypeVar("Result")

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
assert module.RESULT.__name__ == "Result"
