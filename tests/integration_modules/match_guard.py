def probe(value):
    match value:
        case iterable if not hasattr(iterable, "__next__"):
            return f"no next for {type(iterable).__name__}"
        case _:
            return "has next"

# diet-python: validate

from __future__ import annotations
from ._integration import transformed_module

def validate(module):
    assert module.probe([1, 2, 3]) == "no next for list"
    assert module.probe(iter([1, 2, 3])) == "has next"
