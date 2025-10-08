from __future__ import annotations

import sys
from pathlib import Path

import pytest


@pytest.mark.integration
@pytest.mark.skipif(
    not (Path(__file__).resolve().parents[1] / "cpython" / "Lib").exists(),
    reason="CPython stdlib checkout not available",
)
def test_generic_io_annotation_executes(run_integration_module):
    cpython_lib = Path(__file__).resolve().parents[1] / "cpython" / "Lib"
    prev_sys_path = list(sys.path)
    prev_typing = sys.modules.pop("typing", None)
    try:
        sys.path.insert(0, str(cpython_lib))
        with run_integration_module("generic_io_typing") as module:
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
