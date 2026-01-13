from __future__ import annotations


def test_mock_class_property(run_integration_module):
    with run_integration_module("mock_class_property") as module:
        assert module.RESULT is True
