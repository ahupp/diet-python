from __future__ import annotations

import sys

dp = __import__("__dp__")


def exercise():
    original_limit = sys.getrecursionlimit()
    sys.setrecursionlimit(50)

    def recurse():
        return recurse()

    try:
        try:
            recurse()
        except RecursionError:
            try:
                dp.current_exception()
            except RecursionError:
                return False
            return True

        return False
    finally:
        sys.setrecursionlimit(original_limit)

# diet-python: validate

def validate_module(module):

    assert module.exercise() is True
