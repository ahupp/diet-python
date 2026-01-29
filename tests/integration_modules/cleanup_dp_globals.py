def has_dp_name() -> bool:
    return "_dp_name" in globals()

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
assert module.has_dp_name() is False
