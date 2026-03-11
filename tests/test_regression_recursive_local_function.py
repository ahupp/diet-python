from tests._integration import transformed_module


def test_recursive_local_function_keeps_closure_cell_binding(tmp_path):
    source = """
import sys


def exercise():
    original_limit = sys.getrecursionlimit()
    sys.setrecursionlimit(50)

    def recurse():
        return recurse()

    try:
        try:
            recurse()
        except RecursionError:
            return True
        return False
    finally:
        sys.setrecursionlimit(original_limit)
"""

    with transformed_module(tmp_path, "recursive_local_function", source) as module:
        assert module.exercise() is True
