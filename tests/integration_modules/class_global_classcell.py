from __future__ import annotations


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

# diet-python: validate

from __future__ import annotations

def validate(module):
    value, global_value, cls = module.exercise()
    assert value is cls
    assert global_value == 42
    assert "__class__" not in module.__dict__
