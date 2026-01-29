from __future__ import annotations

import collections
import sys
from typing import TYPE_CHECKING, Generic, NamedTuple, TypeVar, final

AnyStr = TypeVar("AnyStr", str, bytes)

if sys.version_info >= (3, 11) or TYPE_CHECKING:

    @final
    class CaptureResult(NamedTuple, Generic[AnyStr]):
        """The result of the capture helper."""

        out: AnyStr
        err: AnyStr

else:

    class CaptureResult(
        collections.namedtuple("CaptureResult", ["out", "err"]),  # noqa: PYI024
        Generic[AnyStr],
    ):
        __slots__ = ()


RESULT = CaptureResult("out", "err")

# diet-python: validate

from __future__ import annotations

import pytest

module = __import__("sys").modules[__name__]
assert module.RESULT.out == "out"
assert module.RESULT.err == "err"
