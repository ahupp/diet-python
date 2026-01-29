results = {}

x = "module"


class C2:
    global x
    x = "class-global"
    y = "class-attr"


result = (x, getattr(C2, "x", None), C2.y)


# diet-python: validate

from __future__ import annotations


module = __import__("sys").modules[__name__]
assert module.result == ("class-global", None, "class-attr")
