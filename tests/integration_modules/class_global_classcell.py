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
