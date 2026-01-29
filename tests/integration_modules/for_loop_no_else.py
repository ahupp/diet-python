from __future__ import annotations


def exercise():
    out = []
    for x in (1,):
        out.append(x)
    return out

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
assert module.exercise() == [1]
