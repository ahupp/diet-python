import gc


def run():
    exc = None
    try:
        raise RuntimeError("boom")
    except RuntimeError as e:
        exc = e
    return gc.get_referrers(exc)

# diet-python: validate

from __future__ import annotations

import pytest

module = __import__("sys").modules[__name__]
referrers = module.run()

assert referrers == []
