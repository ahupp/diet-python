class Example:
    def run(self):
        run = 1
        return run

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
instance = module.Example()
assert instance.run() == 1
