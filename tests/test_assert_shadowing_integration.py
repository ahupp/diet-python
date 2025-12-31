from __future__ import annotations


def test_assert_shadowing(run_integration_module):
    with run_integration_module("assert_shadowing") as module:
        exc = module.trigger()
        assert isinstance(exc, AssertionError)
        assert str(exc) == "hello"
