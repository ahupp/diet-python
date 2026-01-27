from tests._integration import transformed_module


def test_class_body_loads_module_globals(tmp_path):
    source = """
class Example:
    a = __name__
"""
    with transformed_module(tmp_path, "class_body_globals", source) as module:
        assert module.Example.a == module.__name__
