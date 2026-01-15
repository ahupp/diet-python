from __future__ import annotations

import annotationlib


def test_class_annotations_forwardref(run_integration_module):
    with run_integration_module("class_annotations_forwardref") as module:
        annotations = annotationlib.get_annotations(
            module.ThemeSection,
            format=annotationlib.Format.FORWARDREF,
        )
        assert "__dataclass_fields__" in annotations
        assert isinstance(annotations["__dataclass_fields__"], annotationlib.ForwardRef)
