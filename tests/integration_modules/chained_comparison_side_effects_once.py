calls = []


def value() -> int:
    calls.append("hit")
    return 1


def probe() -> list[str]:
    calls.clear()
    if 0 <= value() <= 2:
        return list(calls)
    return list(calls)

# diet-python: validate

from __future__ import annotations

def validate(module):
    assert module.probe() == ["hit"]
