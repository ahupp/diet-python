from tests._integration import transformed_module


def test_functiontype_injects_dp_globals(tmp_path):
    source = """
import types


def make_inner():
    def inner():
        return 1 + 1
    return inner


def run():
    inner_code = make_inner().__code__
    func = types.FunctionType(inner_code, {})
    return func()
"""
    with transformed_module(tmp_path, "functiontype_globals", source) as module:
        assert module.run() == 2
