results = {}

x = "module"


def outer_with_class_method_reads_global():
    x = "outer"

    class Inner:
        x = "class"

        def read():
            return x

    return (Inner.x, Inner.read(), x)


result = outer_with_class_method_reads_global()


# diet-python: validate

from __future__ import annotations


module = __import__("sys").modules[__name__]
assert module.result == ("class", "outer", "outer")
