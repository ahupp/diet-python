import unittest


def exercise():
    try:
        import _testinternalcapi
    except ImportError:
        return "ok"
    _testinternalcapi.get_configs()
    return "ok"
