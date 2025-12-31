import asyncio


async def gather_once() -> list[int]:
    queue: asyncio.Queue[int] = asyncio.Queue()
    await queue.put(1)
    return [await queue.get() for _ in range(1)]
