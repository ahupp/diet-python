from __future__ import annotations
from ._integration import transformed_module

def test_guard_bindings_are_available(run_integration_module):
    with run_integration_module("match_guard") as module:
        assert module.probe([1, 2, 3]) == "no next for list"
        assert module.probe(iter([1, 2, 3])) == "has next"
