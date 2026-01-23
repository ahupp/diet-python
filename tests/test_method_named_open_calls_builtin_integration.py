from __future__ import annotations


def test_method_named_open_calls_builtin(run_integration_module):
    with run_integration_module("method_named_open_calls_builtin") as module:
        assert module.RESULT == "payload"
