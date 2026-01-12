from __future__ import annotations


def test_class_lookup_lambda_avoids_recursion(run_integration_module):
    with run_integration_module("class_lookup_lambda_recursion") as module:
        assert module.RESULT == module.__name__
