import types


def make():
    return (x for x in range(2))


def main():
    g = make()
    gen_func = types.FunctionType(g.gi_code, {})
    return list(gen_func([1, 2]))


# diet-python: validate

def validate_module(module):
    import pytest

    with pytest.raises(TypeError, match=r"object is not an iterator"):
        module.main()
