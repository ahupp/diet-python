from __future__ import annotations

from pathlib import Path
import inspect

import pytest

from tests._integration import integration_module


@pytest.mark.parametrize("mode", ["transform"], ids=["transform"])
def test_asyncio_iscoroutinefunction(tmp_path: Path, mode: str) -> None:
    if mode == "transform":
        pytest.xfail(
            "transformed async defs are not yet marked as coroutine functions "
            "for inspect/asyncio iscoroutinefunction checks"
        )
    source = """
async def coro():
    return 1

class C:
    async def method(self):
        return 2
"""
    with integration_module(tmp_path, "asyncio_iscoroutinefunction", source, mode=mode) as module:
        assert inspect.iscoroutinefunction(module.coro)
        assert inspect.iscoroutinefunction(module.C.method)
        assert inspect.iscoroutinefunction(module.C().method)
