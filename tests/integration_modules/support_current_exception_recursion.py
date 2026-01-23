from __future__ import annotations

import sys
import __dp__


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
                __dp__.current_exception()
            except RecursionError:
                return False
            return True

        return False
    finally:
        sys.setrecursionlimit(original_limit)

# diet-python: validate

from __future__ import annotations

def validate(module):
    assert module.exercise() is True
