from __future__ import annotations


def test_class_delayed_classcell(run_integration_module):
    with run_integration_module("class_delayed_classcell") as module:
        value, cls, class_value = module.exercise()
        assert value is None
        assert class_value is cls
