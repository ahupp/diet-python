from __future__ import annotations


def test_function_local_annotation(run_integration_module):
    with run_integration_module("function_local_annotation") as module:
        assert module.exercise() == 1
