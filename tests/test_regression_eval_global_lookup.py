from tests._integration import integration_module


def test_eval_function_resolves_module_global(tmp_path):
    source = """
VALUE = 7


def read_value():
    return VALUE
"""
    with integration_module(tmp_path, "eval_global_lookup", source, mode="eval") as module:
        assert module.read_value() == 7
