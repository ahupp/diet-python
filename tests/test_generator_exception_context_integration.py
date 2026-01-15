from __future__ import annotations


def test_generator_exception_context(run_integration_module):
    with run_integration_module("generator_exception_context") as module:
        exc_type, args = module.exercise()
        assert exc_type is KeyError
        assert args == ("a",)
