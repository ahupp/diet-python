from __future__ import annotations

import asyncio


class CM:
    async def __aenter__(self):
        return "entered"


async def run():
    async with CM():
        return "body"


def get_result():
    try:
        asyncio.run(run())
    except TypeError as exc:
        return str(exc)
    return "no error"
