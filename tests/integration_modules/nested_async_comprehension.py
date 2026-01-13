from __future__ import annotations

import asyncio


async def asynciter(items):
    for item in items:
        await asyncio.sleep(0)
        yield item


async def nested():
    return [[i + j async for i in asynciter([1, 2])] for j in [10, 20]]


async def gen_inside_gen():
    gens = ((i async for i in asynciter(range(j))) for j in [3, 5])
    return [x for g in gens async for x in g]


def get_values():
    return asyncio.run(nested())


def get_gen_values():
    return asyncio.run(gen_inside_gen())
