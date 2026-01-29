class Example:
    SENTINEL = object()

    def method(self, value=SENTINEL):
        return value

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
instance = module.Example()
assert instance.method() is module.Example.SENTINEL
