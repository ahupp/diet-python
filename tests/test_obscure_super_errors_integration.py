from __future__ import annotations


def test_obscure_super_errors(run_integration_module):
    with run_integration_module("obscure_super_errors") as module:
        exc_type, message = module.exercise()
        assert exc_type is RuntimeError
        assert "empty __class__ cell" in message
