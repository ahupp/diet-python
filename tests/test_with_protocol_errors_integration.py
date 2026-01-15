from __future__ import annotations


def test_with_protocol_errors(run_integration_module):
    with run_integration_module("with_protocol_errors") as module:
        errors = module.exercise()
        assert errors[0][0] is TypeError
        assert "context manager" in errors[0][1]
        assert errors[1][0] is TypeError
        assert "__exit__" in errors[1][1]
