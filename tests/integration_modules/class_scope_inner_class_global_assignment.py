results = {}

x = "module"


def outer_with_inner_class_global_assignment():
    x = "outer"

    class Inner:
        global x
        x = "class-global"
        y = "class-attr"

    return (x, getattr(Inner, "x", None), Inner.y)


result = (outer_with_inner_class_global_assignment(), x)


# diet-python: validate

from __future__ import annotations


module = __import__("sys").modules[__name__]
assert module.result == (("outer", None, "class-attr"), "class-global")
