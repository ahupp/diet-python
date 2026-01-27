from tests._integration import transformed_module


def test_nested_class_qualname_uses_original_method_name(tmp_path):
    source = """
from typing import Any

class Container:
    def make(self) -> str:
        class Sub(Any):
            pass
        return repr(Sub)

VALUE = Container().make()
"""
    with transformed_module(tmp_path, "nested_class_qualname", source) as module:
        assert "Container.make.<locals>.Sub" in module.VALUE
