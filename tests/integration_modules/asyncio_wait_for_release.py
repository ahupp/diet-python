from __future__ import annotations

import asyncio
import gc
import weakref


class Payload:
    pass


async def hold_ref(ref_holder):
    obj = Payload()
    ref_holder.append(weakref.ref(obj))
    await asyncio.sleep(10)


def leak_check():
    ref_holder = []

    async def runner():
        await asyncio.wait_for(hold_ref(ref_holder), 0.01)

    try:
        asyncio.run(runner())
    except asyncio.TimeoutError:
        pass

    gc.collect()
    return ref_holder[0]()
