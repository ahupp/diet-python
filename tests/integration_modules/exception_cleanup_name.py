from __future__ import annotations


def has_exception_name():
    try:
        1 / 0
    except Exception as e:
        pass
    return "e" in locals()

# diet-python: validate

from __future__ import annotations

def validate(module):
    assert module.has_exception_name() is False
