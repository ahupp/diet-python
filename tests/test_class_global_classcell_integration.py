from __future__ import annotations


def test_class_global_classcell(run_integration_module):
    with run_integration_module("class_global_classcell") as module:
        value, global_value, cls = module.exercise()
        assert value is cls
        assert global_value == 42
        assert "__class__" not in module.__dict__
