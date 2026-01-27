from tests._integration import transformed_module


def test_class_global_classcell_uses_globals_and_classcell(tmp_path):
    source = """
def exercise():
    class X:
        global __class__
        __class__ = 42

        def f(self):
            return __class__

    x = X()
    value = x.f()
    global_value = globals()["__class__"]
    del globals()["__class__"]
    return value, global_value, X
"""
    with transformed_module(tmp_path, "class_global_classcell", source) as module:
        value, global_value, cls = module.exercise()
        assert value is cls
        assert global_value == 42
        assert "__class__" not in module.__dict__
