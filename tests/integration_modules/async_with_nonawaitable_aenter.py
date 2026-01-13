from __future__ import annotations

import asyncio


class CM:
    def __aenter__(self):
        return 1

    async def __aexit__(self, exc_type, exc, tb):
        return False


async def run():
    async with CM():
        return "body"


def get_error():
    try:
        asyncio.run(run())
    except TypeError as exc:
        return str(exc)
    return "no error"
