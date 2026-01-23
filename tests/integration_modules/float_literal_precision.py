VALUE = 0.9999999999999999
RESULT = VALUE < 1.0

# diet-python: validate

from __future__ import annotations

def validate(module):
    assert module.RESULT is True
