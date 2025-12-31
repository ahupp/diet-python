from __future__ import annotations


def test_float_literal_precision(run_integration_module):
    with run_integration_module("float_literal_precision") as module:
        assert module.RESULT is True
