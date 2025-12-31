from __future__ import annotations


def test_typevar_tuple_default_none(run_integration_module):
    with run_integration_module("type_param_typevar_tuple_default_none") as module:
        assert module.A.__name__ == "A"
