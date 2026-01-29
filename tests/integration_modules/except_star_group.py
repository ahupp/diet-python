from __future__ import annotations


def handle():
    exc = None
    try:
        raise ExceptionGroup("eg", [OSError("boom")])
    except* OSError as excs:
        exc = excs
    return exc

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
exc = module.handle()
assert exc is not None
assert isinstance(exc, ExceptionGroup)
assert exc.exceptions
assert isinstance(exc.exceptions[0], OSError)
