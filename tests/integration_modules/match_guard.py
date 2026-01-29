def probe(value):
    match value:
        case iterable if not hasattr(iterable, "__next__"):
            return f"no next for {type(iterable).__name__}"
        case _:
            return "has next"

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
assert module.probe([1, 2, 3]) == "no next for list"
assert module.probe(iter([1, 2, 3])) == "has next"
