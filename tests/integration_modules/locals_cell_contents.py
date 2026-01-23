def outer():
    x = 2
    def inner(y):
        z = x + y
        return locals()
    return inner(4)

# diet-python: validate

from __future__ import annotations

def validate(module):
    result = module.outer()
    assert result == {"x": 2, "y": 4, "z": 6}
