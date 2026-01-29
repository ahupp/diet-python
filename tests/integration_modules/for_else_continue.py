def collect_for_else_continue():
    seen = []
    for outer in range(3):
        for _inner in []:
            seen.append((_inner, outer))
        else:
            seen.append(outer)
            continue
        seen.append("unreachable")
    return seen


RESULT = collect_for_else_continue()

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
assert module.RESULT == [0, 1, 2]
