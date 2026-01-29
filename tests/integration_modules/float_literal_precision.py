VALUE = 0.9999999999999999
RESULT = VALUE < 1.0

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
assert module.RESULT is True
