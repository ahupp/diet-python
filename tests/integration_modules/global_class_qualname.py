def make_name():
    global Y

    class Y:
        class Inner:
            pass

    return Y.__qualname__, Y.Inner.__qualname__

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
qualname, inner_qualname = module.make_name()
assert qualname == "Y"
assert inner_qualname == "Y.Inner"
