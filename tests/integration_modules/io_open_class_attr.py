import io


class Reader:
    open = io.open


def read_self():
    with Reader().open(__file__, "rb") as handle:
        return handle.read(1)


RESULT = read_self()

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
assert isinstance(module.RESULT, bytes)
assert len(module.RESULT) == 1
