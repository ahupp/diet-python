from __future__ import annotations


def test_typing_import_module(run_integration_module):
    with run_integration_module("typing_import") as module:
        assert module.RESULT.__name__ == "Result"
