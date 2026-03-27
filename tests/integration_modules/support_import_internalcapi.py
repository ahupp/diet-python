import unittest


def exercise():
    try:
        import _testinternalcapi
    except ImportError:
        return "ok"
    _testinternalcapi.get_configs()
    return "ok"

# diet-python: validate

def validate_module(module):

    assert module.exercise() == "ok"
