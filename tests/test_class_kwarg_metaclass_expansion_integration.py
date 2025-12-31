from __future__ import annotations


def test_class_kwarg_metaclass_expansion(run_integration_module):
    with run_integration_module("class_kwarg_metaclass_expansion") as module:
        assert module.RESULT == ((), {})
