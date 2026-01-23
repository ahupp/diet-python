class Example:
    value = __name__


RESULT = Example.value

# diet-python: validate

from __future__ import annotations

def validate(module):
    assert module.RESULT == module.__name__
