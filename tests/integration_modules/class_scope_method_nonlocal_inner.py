class C4:
    def outer():
        x = "outer"

        def inner():
            nonlocal x
            x = "inner"

        inner()
        return x


result = C4.outer()


# diet-python: validate

from __future__ import annotations


module = __import__("sys").modules[__name__]
assert module.result == "inner"
