import unittest


def exercise():
    try:
        import _testinternalcapi
    except ImportError:
        return "ok"
    _testinternalcapi.get_configs()
    return "ok"

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
assert module.exercise() == "ok"
