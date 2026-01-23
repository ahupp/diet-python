try:
    A, B, C = map(int, (1, 2, 3))
except Exception as exc:  # pragma: no cover - reproduction module
    RESULT = ("error", type(exc).__name__, str(exc))
else:  # pragma: no cover
    RESULT = ("ok", (A, B, C))

# diet-python: validate

import pytest

def validate(module):
    assert module.RESULT[0] == "ok"
    assert module.RESULT[1] == (1, 2, 3)
