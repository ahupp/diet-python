import pytest

from tests._integration import transformed_module


def test_genexpr_requires_iterator(tmp_path):
    source = """
import types

def make():
    return (x for x in range(2))

def main():
    g = make()
    gen_func = types.FunctionType(g.gi_code, {})
    return list(gen_func([1, 2]))
"""
    with transformed_module(tmp_path, "genexpr_iterator_semantics", source) as module:
        with pytest.raises(TypeError, match=r"object is not an iterator"):
            module.main()
