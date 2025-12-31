from __future__ import annotations


def test_io_open_class_attr_is_callable(run_integration_module):
    with run_integration_module("io_open_class_attr") as module:
        assert isinstance(module.RESULT, bytes)
        assert len(module.RESULT) == 1
