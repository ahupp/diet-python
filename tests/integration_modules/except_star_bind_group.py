from __future__ import annotations


def handle():
    captured = None
    try:
        raise ExceptionGroup("eg", [OSError("boom")])
    except* OSError as excs:
        captured = excs
    return captured


# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
exc = module.handle()
assert exc is not None
assert isinstance(exc, ExceptionGroup)
assert isinstance(exc.exceptions[0], OSError)
