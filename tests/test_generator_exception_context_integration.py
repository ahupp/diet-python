from __future__ import annotations


def test_generator_exception_context(run_integration_module):
    with run_integration_module("generator_exception_context") as module:
        context_type, context_args = module.exception_context()
        assert context_type is KeyError
        assert context_args == ("a",)
