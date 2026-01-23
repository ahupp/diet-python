from __future__ import annotations


def exercise():
    def inner():
        value: int = 1
        return value

    return inner()

# diet-python: validate

from __future__ import annotations

def validate(module):
    assert module.exercise() == 1
