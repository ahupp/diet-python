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

# diet-python: validate

module = __import__("sys").modules[__name__]
assert module.get_values() == [[11, 12], [21, 22]]
assert module.get_gen_values() == [0, 1, 2, 0, 1, 2, 3, 4]
