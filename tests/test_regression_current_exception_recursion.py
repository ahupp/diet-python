from tests._integration import transformed_module


def test_current_exception_recursion_regression(tmp_path):
    source = """
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
"""

    with transformed_module(
        tmp_path, "current_exception_recursion_regression", source
    ) as module:
        assert module.exercise() is True
