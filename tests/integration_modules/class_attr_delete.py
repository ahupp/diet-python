class Example:
    value = 1
    del value


EXPECTS_VALUE = hasattr(Example, "value")

# diet-python: validate

from __future__ import annotations

def validate(module):
    assert module.EXPECTS_VALUE is False
