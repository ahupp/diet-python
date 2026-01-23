from __future__ import annotations


def test_super_empty_classcell(run_integration_module):
    with run_integration_module("super_empty_classcell") as module:
        exc_type, message = module.exercise()
        assert exc_type is RuntimeError
        assert "empty __class__ cell" in message
