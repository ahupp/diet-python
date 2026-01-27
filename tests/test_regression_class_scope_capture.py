from tests._integration import transformed_module


def test_class_scope_captures_enclosing_value(tmp_path):
    source = """
def outer():
    x = "outer"

    class Inner:
        y = x

    return Inner.y
"""
    with transformed_module(tmp_path, "class_scope_capture", source) as module:
        assert module.outer() == "outer"
