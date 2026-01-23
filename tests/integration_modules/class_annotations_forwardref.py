class ThemeSection:
    if False:
        from typing import ClassVar
    __dataclass_fields__: ClassVar[int]

# diet-python: validate

from __future__ import annotations

import annotationlib

def validate(module):
    annotations = annotationlib.get_annotations(
    module.ThemeSection,
    format=annotationlib.Format.FORWARDREF,
    )
    assert "__dataclass_fields__" in annotations
    assert isinstance(annotations["__dataclass_fields__"], annotationlib.ForwardRef)
