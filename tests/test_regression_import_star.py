from tests._integration import transformed_module


def test_import_star_module_level(tmp_path):
    source = """
from math import *
VALUE = sin(pi / 2)
"""
    with transformed_module(tmp_path, "import_star_math", source) as module:
        assert module.VALUE == 1.0
