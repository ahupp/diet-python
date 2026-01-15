from __future__ import annotations


def test_operator_all_module_attrs(run_integration_module):
    with run_integration_module("operator_all_module_attrs") as module:
        assert module.exercise() == []
