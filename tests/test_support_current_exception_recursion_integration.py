from __future__ import annotations


def test_support_current_exception_recursion(run_integration_module):
    with run_integration_module("support_current_exception_recursion") as module:
        assert module.exercise() is True
