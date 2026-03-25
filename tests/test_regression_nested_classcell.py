from tests._integration import transformed_module


def test_nested_function_can_capture_method_dunder_class(tmp_path):
    source = """
def exercise():
    class C:
        def f(self):
            def g():
                return __class__
            return g()

    return C().f(), C
"""
    with transformed_module(tmp_path, "nested_classcell_capture", source) as module:
        value, cls = module.exercise()
        assert value is cls
