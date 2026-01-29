results = {}

x = "global"


class C1:
    x = "class"

    def read():
        return x


result = (C1.x, C1.read(), x)


# diet-python: validate

from __future__ import annotations


module = __import__("sys").modules[__name__]
assert module.result == ("class", "global", "global")
