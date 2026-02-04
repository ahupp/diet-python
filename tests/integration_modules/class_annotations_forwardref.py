class ThemeSection:
    if False:
        from typing import ClassVar
    __dataclass_fields__: ClassVar[int]

# diet-python: validate

from __future__ import annotations

import annotationlib

module = __import__("sys").modules[__name__]
annotations = annotationlib.get_annotations(
module.ThemeSection,
format=annotationlib.Format.FORWARDREF,
)
assert "__dataclass_fields__" in annotations
value = annotations["__dataclass_fields__"]
assert isinstance(value, annotationlib.ForwardRef)
assert value.__forward_arg__.startswith("ClassVar[")
