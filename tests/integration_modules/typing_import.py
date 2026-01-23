from typing import TypeVar


RESULT = TypeVar("Result")

# diet-python: validate

from __future__ import annotations

def validate(module):
    assert module.RESULT.__name__ == "Result"
