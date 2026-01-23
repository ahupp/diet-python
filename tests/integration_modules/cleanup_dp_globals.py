def has_dp_name() -> bool:
    return "_dp_name" in globals()

# diet-python: validate

from __future__ import annotations

def validate(module):
    assert module.has_dp_name() is False
