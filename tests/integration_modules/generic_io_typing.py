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
try:
    import tests as _tests_pkg
    repo_root = Path(_tests_pkg.__file__).resolve().parents[1]
except Exception:
    repo_root = Path(__file__).resolve().parents[2]
cpython_lib = repo_root / "cpython" / "Lib"
if not cpython_lib.exists():
    cpython_lib = repo_root.parent / "cpython" / "Lib"
if not cpython_lib.exists():
    pytest.skip("CPython stdlib checkout not available")
prev_sys_path = list(sys.path)
prev_typing = sys.modules.pop("typing", None)
try:
    sys.path.insert(0, str(cpython_lib))
    example = module.Example
    readlines = example.readlines
    annotations = readlines.__annotations__
    ann = annotations["return"]
    assert getattr(ann, "__origin__", None) is list
    assert getattr(ann, "__args__", None) == (module.AnyStr,)
    orig_base = example.__orig_bases__[0]
    assert orig_base.__origin__ is module.Generic
    assert orig_base.__args__ == (module.AnyStr,)
finally:
    sys.path[:] = prev_sys_path
    if prev_typing is not None:
        sys.modules["typing"] = prev_typing
    else:
        sys.modules.pop("typing", None)
