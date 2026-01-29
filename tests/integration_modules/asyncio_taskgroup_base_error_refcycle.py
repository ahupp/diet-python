import asyncio
import gc
import types


class BaseError(BaseException):
    pass


async def _run_taskgroup():
    exc = BaseError()
    try:
        async with asyncio.TaskGroup() as tg:
            async def boom():
                raise exc
            tg.create_task(boom())
    except* BaseError:
        pass
    return exc


def referrer_frames():
    exc = asyncio.run(_run_taskgroup())
    gc.collect()
    return [ref for ref in gc.get_referrers(exc) if isinstance(ref, types.FrameType)]

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
assert module.referrer_frames() == []
