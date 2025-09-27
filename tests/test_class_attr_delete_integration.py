from __future__ import annotations

def test_class_attr_delete_removes_attribute(run_integration_module):
    with run_integration_module("class_attr_delete") as module:
        assert module.EXPECTS_VALUE is False
