from __future__ import annotations


def exercise():
    errors = []
    try:
        raise ValueError("boom")
    except ValueError:
        errors.append("caught")
    else:
        errors.append("else")
    return errors


# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
assert module.exercise() == ["caught"]
