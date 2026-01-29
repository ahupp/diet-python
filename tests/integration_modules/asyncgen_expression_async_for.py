from __future__ import annotations

import asyncio


async def arange(n):
    for i in range(n):
        await asyncio.sleep(0)
        yield i


def make_arange(n):
    return (i * 2 async for i in arange(n))


async def run():
    return [i async for i in make_arange(3)]


def get_values():
    return asyncio.run(run())

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
assert module.get_values() == [0, 2, 4]
