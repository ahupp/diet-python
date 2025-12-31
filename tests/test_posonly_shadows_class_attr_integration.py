from __future__ import annotations


def test_posonly_param_shadows_class_attr(run_integration_module):
    with run_integration_module("posonly_shadows_class_attr") as module:
        assert module.make_value() == 3
