def gen():
    value = yield "start"
    yield value


# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
g = module.gen()
assert next(g) == "start"
assert g.send("x") == "x"
