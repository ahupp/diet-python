class Example:
    def close(self):
        close = lambda: "ok"
        if close:
            return close()
        return "no"

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
instance = module.Example()
assert instance.close() == "ok"
