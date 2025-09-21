from __future__ import annotations

from ._integration import transformed_module

MODULE_SOURCE = """
def probe(value):
    match value:
        case iterable if not hasattr(iterable, "__next__"):
            return f"no next for {type(iterable).__name__}"
        case _:
            return "has next"
"""


def test_guard_bindings_are_available(tmp_path):
    with transformed_module(tmp_path, "match_guard", MODULE_SOURCE) as module:
        assert module.probe([1, 2, 3]) == "no next for list"
        assert module.probe(iter([1, 2, 3])) == "has next"
