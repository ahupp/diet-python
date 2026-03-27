from tests._integration import transformed_module


def test_transformed_function_uses_live_defaults_fields(tmp_path):
    source = """
def make():
    def inner(a=1, *, b=2):
        return a, b
    return inner

def run():
    inner = make()
    inner.__defaults__ = (10,)
    inner.__kwdefaults__ = {"b": 20}
    return inner()
"""

    with transformed_module(tmp_path, "mutated_function_defaults", source) as module:
        assert module.run() == (10, 20)


def test_closure_wrapped_function_uses_live_defaults_fields(tmp_path):
    source = """
def make():
    sentinel = object()
    def inner(value=sentinel):
        return value
    return inner

def run():
    inner = make()
    replacement = object()
    inner.__defaults__ = (replacement,)
    return inner() is replacement
"""

    with transformed_module(tmp_path, "mutated_closure_function_defaults", source) as module:
        assert module.run() is True
