from __future__ import annotations

from typing import TYPE_CHECKING

SENTINEL = object()


class Marker:
    if TYPE_CHECKING:
        typed_attr: int
        other_attr: str

    value = SENTINEL
