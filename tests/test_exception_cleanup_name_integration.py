from __future__ import annotations


def test_exception_cleanup_name(run_integration_module):
    with run_integration_module("exception_cleanup_name") as module:
        assert module.has_exception_name() is False
