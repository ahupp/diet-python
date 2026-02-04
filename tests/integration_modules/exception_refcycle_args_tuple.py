import gc


def run():
    exc = None
    try:
        raise RuntimeError("boom")
    except RuntimeError as err:
        exc = err
    return gc.get_referrers(exc)


# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
assert module.run() == []
