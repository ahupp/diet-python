results = {}

x = "module"


class C3:
    def set_x():
        global x
        x = "method-global"

    def read_x():
        return x


C3.set_x()
result = (x, C3.read_x())


# diet-python: validate

from __future__ import annotations


module = __import__("sys").modules[__name__]
assert module.result == ("method-global", "method-global")
