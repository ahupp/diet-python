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

def validate(module):
    before, after = module.run()
    assert before == after
