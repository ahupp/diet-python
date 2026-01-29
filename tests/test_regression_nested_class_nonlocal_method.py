from tests._integration import transformed_module


def test_nested_class_method_with_nonlocal(tmp_path):
    source = """
class Outer:
    def run(self):
        counter = 0

        class Inner:
            def bump(self):
                nonlocal counter
                counter += 1

        Inner().bump()
        return counter
"""
    with transformed_module(tmp_path, "nested_class_nonlocal_method", source) as module:
        assert module.Outer().run() == 1
