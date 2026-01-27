from tests._integration import transformed_module


def test_class_method_decorator_rewrites(tmp_path):
    source = """
class Example:
    @property
    def value(self):
        return 42
"""
    with transformed_module(tmp_path, "class_method_decorators", source) as module:
        assert module.Example().value == 42
