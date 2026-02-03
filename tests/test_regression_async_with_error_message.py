import pytest

from tests._integration import transformed_module


def test_async_with_aenter_error_message(tmp_path):
    source = """
import asyncio

class BadEnter:
    def __aenter__(self):
        return 1

    async def __aexit__(self, exc_type, exc, tb):
        return False

async def run():
    async with BadEnter():
        return "ok"

def main():
    return asyncio.run(run())
"""
    with transformed_module(tmp_path, "bad_async_enter", source) as module:
        with pytest.raises(
            TypeError,
            match=r"'async with' received an object from __aenter__ that does not implement __await__: int",
        ):
            module.main()


def test_async_with_aexit_error_message(tmp_path):
    source = """
import asyncio

class BadExit:
    async def __aenter__(self):
        return "ok"

    def __aexit__(self, exc_type, exc, tb):
        return 1

async def run():
    async with BadExit():
        return "ok"

def main():
    return asyncio.run(run())
"""
    with transformed_module(tmp_path, "bad_async_exit", source) as module:
        with pytest.raises(
            TypeError,
            match=r"'async with' received an object from __aexit__ that does not implement __await__: int",
        ):
            module.main()
