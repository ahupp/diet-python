from __future__ import annotations

from pathlib import Path

import pytest

from tests._integration import integration_module


@pytest.mark.parametrize("mode", ["eval"], ids=["eval"])
def test_eval_clears_exception_indicator_in_async(tmp_path: Path, mode: str) -> None:
    source = """
import asyncio
import ctypes
import pytest
import __dp__

async def main():
    err = ctypes.pythonapi.PyErr_Occurred
    err.restype = ctypes.c_void_p
    orig_is = __dp__.is_

    def is_check(lhs, rhs):
        assert err() is None
        return orig_is(lhs, rhs)

    __dp__.is_ = is_check
    try:
        async def afunc():
            await asyncio.sleep(0.1)

        coro = afunc()
        task = asyncio.create_task(coro)
        await asyncio.sleep(0)
        with pytest.raises(RuntimeError):
            await coro
        task.cancel()
        return True
    finally:
        __dp__.is_ = orig_is

def run():
    return asyncio.run(main())
"""
    with integration_module(tmp_path, "eval_exception_indicator", source, mode=mode) as module:
        assert module.run() is True


@pytest.mark.parametrize("mode", ["eval"], ids=["eval"])
def test_eval_clears_exception_indicator_in_generator_finally(
    tmp_path: Path, mode: str
) -> None:
    source = """
import contextlib
import os
import tempfile

@contextlib.contextmanager
def chdir_cm():
    cwd = os.getcwd()
    os.chdir(tempfile.gettempdir())
    try:
        yield
    finally:
        os.chdir(cwd)

def run():
    try:
        with chdir_cm():
            raise SystemExit(0)
    except SystemExit:
        return "ok"
"""
    with integration_module(tmp_path, "eval_exception_indicator_gen_finally", source, mode=mode) as module:
        assert module.run() == "ok"
