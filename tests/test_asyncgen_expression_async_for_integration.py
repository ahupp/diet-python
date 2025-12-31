from __future__ import annotations


def test_asyncgen_expression_async_for(run_integration_module):
    with run_integration_module("asyncgen_expression_async_for") as module:
        assert module.get_values() == [0, 2, 4]
