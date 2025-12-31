from __future__ import annotations


def test_with_context_releases_exception_refs(run_integration_module):
    with run_integration_module("with_context_exception_leak") as module:
        assert module.leak_check() is None
