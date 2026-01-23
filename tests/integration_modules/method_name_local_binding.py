class Example:
    def close(self):
        close = lambda: "ok"
        if close:
            return close()
        return "no"

# diet-python: validate

from __future__ import annotations

def validate(module):
    instance = module.Example()
    assert instance.close() == "ok"
