from typing import Generic, List, TypeVar

AnyStr = TypeVar("AnyStr", str, bytes)


class Example(Generic[AnyStr]):
    def readlines(self) -> List[AnyStr]:
        ...

# diet-python: validate

from __future__ import annotations

import sys
from pathlib import Path

import pytest


module = __import__("sys").modules[__name__]
cpython_lib = Path(__file__).resolve().parents[1] / "cpython" / "Lib"
if not cpython_lib.exists():
    pytest.skip("CPython stdlib checkout not available")
prev_sys_path = list(sys.path)
prev_typing = sys.modules.pop("typing", None)
try:
    sys.path.insert(0, str(cpython_lib))
    example = module.Example
    readlines = example.readlines
    annotations = readlines.__annotations__
    assert annotations["return"] == module.List[module.AnyStr]
    assert example.__orig_bases__[0] == module.Generic[module.AnyStr]
finally:
    sys.path[:] = prev_sys_path
    if prev_typing is not None:
        sys.modules["typing"] = prev_typing
    else:
        sys.modules.pop("typing", None)
