from __future__ import annotations

from typing import TYPE_CHECKING

SENTINEL = object()


class Marker:
    if TYPE_CHECKING:
        typed_attr: int
        other_attr: str

    value = SENTINEL

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
assert module.Marker.value is module.SENTINEL
assert not hasattr(module.Marker, "typed_attr")
