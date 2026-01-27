from tests._integration import transformed_module


def test_class_scope_comprehension_uses_inner_binding(tmp_path):
    source = """
class Example:
    values = [lc for lc in range(3)]
"""
    with transformed_module(tmp_path, "class_scope_comprehension", source) as module:
        assert module.Example.values == [0, 1, 2]
