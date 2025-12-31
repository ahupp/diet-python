from __future__ import annotations


def test_except_star_group_returns_exception_group(run_integration_module):
    with run_integration_module("except_star_group") as module:
        exc = module.handle()
        assert exc is not None
        assert isinstance(exc, ExceptionGroup)
        assert exc.exceptions
        assert isinstance(exc.exceptions[0], OSError)
