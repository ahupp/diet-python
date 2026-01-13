from __future__ import annotations

import asyncio


async def run():
    async for _ in (1, 2, 3):
        pass


def get_error():
    try:
        asyncio.run(run())
    except TypeError as exc:
        return str(exc)
    return "no error"
