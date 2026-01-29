
def outer_with_nonlocal_and_inner_class():
    x = "outer"

    def inner():
        nonlocal x
        x = "inner"

        class Inner:
            y = x

        return Inner.y

    y = inner()
    return (x, y)


result = outer_with_nonlocal_and_inner_class()


# diet-python: validate

from __future__ import annotations


module = __import__("sys").modules[__name__]
assert module.result == ("inner", "inner")
