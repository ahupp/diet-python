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

# diet-python: validate

module = __import__("sys").modules[__name__]
message = module.get_result()
assert "asynchronous context manager" in message
assert "__aexit__" in message
