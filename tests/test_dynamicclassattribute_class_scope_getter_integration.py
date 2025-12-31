from __future__ import annotations


def test_dynamicclassattribute_class_scope_getter(run_integration_module):
    with run_integration_module("dynamicclassattribute_class_scope_getter") as module:
        assert module.get_value() == 2
