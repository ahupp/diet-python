from __future__ import annotations


def test_type_checking_annotations_preserve_class_body(run_integration_module):
    with run_integration_module("type_checking_annotations") as module:
        assert module.Marker.value is module.SENTINEL
        assert not hasattr(module.Marker, "typed_attr")
