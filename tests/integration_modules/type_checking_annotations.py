from __future__ import annotations

from typing import TYPE_CHECKING

SENTINEL = object()


class Marker:
    if TYPE_CHECKING:
        typed_attr: int
        other_attr: str

    value = SENTINEL

# diet-python: validate

def validate_module(module):

    assert module.Marker.value is module.SENTINEL
    assert not hasattr(module.Marker, "typed_attr")
