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

def validate(module):
    referrers = module.run()

    assert referrers == []
