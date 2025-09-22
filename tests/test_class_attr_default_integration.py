from __future__ import annotations


def test_class_attribute_default_is_resolved(run_integration_module):
    with run_integration_module("class_attr_default") as module:
        instance = module.Example()
        assert instance.method() is module.Example.SENTINEL
