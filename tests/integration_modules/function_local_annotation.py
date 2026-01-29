from __future__ import annotations


def exercise():
    def inner():
        value: int = 1
        return value

    return inner()

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
assert module.exercise() == 1
