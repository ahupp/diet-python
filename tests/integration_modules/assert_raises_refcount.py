import sys
import unittest


def _boom():
    try:
        raise ValueError("boom")
    except ValueError:
        raise ValueError("boom")


def run():
    case = unittest.TestCase()
    before = sys.getrefcount(_boom)
    case.assertRaises(ValueError, _boom)
    return before, sys.getrefcount(_boom)

# diet-python: validate

module = __import__("sys").modules[__name__]
before, after = module.run()
assert before == after
