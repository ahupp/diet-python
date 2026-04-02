from __future__ import annotations

import asyncio


def build(offset):
    async def agen():
        total = 1
        try:
            sent = yield total + offset
        except ValueError as exc:
            total += exc.args[0]
        else:
            if sent is not None:
                total += sent
        yield total + offset

    return agen()


async def run_send():
    gen = build(10)
    first = await anext(gen)
    second = await gen.asend(2)
    if type(gen).__name__ == "ClosureAsyncGenerator":
        assert gen.ag_frame is None
    return first, second


async def run_throw():
    gen = build(10)
    first = await anext(gen)
    second = await gen.athrow(ValueError(3))
    if type(gen).__name__ == "ClosureAsyncGenerator":
        assert gen.ag_frame is None
    return first, second

# diet-python: validate

def validate_module(module):
    import asyncio

    assert asyncio.run(module.run_send()) == (11, 13)
    assert asyncio.run(module.run_throw()) == (11, 14)
