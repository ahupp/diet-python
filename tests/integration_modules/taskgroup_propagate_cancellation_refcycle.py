import asyncio
import gc


def run():
    exc = None
    try:
        try:
            raise asyncio.CancelledError()
        except asyncio.CancelledError as err:
            raise TimeoutError from err
    except TimeoutError as err:
        exc = err.__cause__
    return gc.get_referrers(exc)

# diet-python: validate

from __future__ import annotations

import pytest

module = __import__("sys").modules[__name__]
referrers = module.run()

assert referrers == []
