from __future__ import annotations


def test_for_else_continue(run_integration_module):
    with run_integration_module("for_else_continue") as module:
        assert module.RESULT == [0, 1, 2]
