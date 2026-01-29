results = {}

z1 = "outer"


class InnerSeesOuterScope:
    z1 = "inner"

    class Inner:
        y = z1


result = InnerSeesOuterScope.Inner.y


# diet-python: validate

from __future__ import annotations


module = __import__("sys").modules[__name__]
assert module.result == "outer"
