import asyncio


async def gather_once() -> list[int]:
    queue: asyncio.Queue[int] = asyncio.Queue()
    await queue.put(1)
    return [await queue.get() for _ in range(1)]

# diet-python: validate

import asyncio

def validate(module):
    assert asyncio.run(module.gather_once()) == [1]
