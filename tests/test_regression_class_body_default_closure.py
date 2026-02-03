from tests._integration import transformed_module


def test_class_body_default_uses_enclosing_var(tmp_path):
    source = """

def make():
    sentinel = object()
    class C:
        def method(self, value=sentinel):
            return value is sentinel
    return C()


def run():
    return make().method()
"""
    with transformed_module(tmp_path, "class_body_default_closure", source) as module:
        assert module.run() is True
