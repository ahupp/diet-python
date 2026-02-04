from tests._integration import integration_module


def test_eval_module_is_visible_to_inspect(tmp_path):
    source = """
VALUE = 1


def func():
    return VALUE
"""
    with integration_module(tmp_path, "eval_inspect_module", source, mode="eval") as module:
        import inspect

        assert inspect.ismodule(module)
        text = inspect.getsource(module)
        assert "def func()" in text
