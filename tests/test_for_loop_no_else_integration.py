from __future__ import annotations


def test_for_loop_no_else_integration(run_integration_module):
    with run_integration_module("for_loop_no_else") as module:
        assert module.exercise() == [1]
