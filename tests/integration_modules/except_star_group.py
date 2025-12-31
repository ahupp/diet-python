from __future__ import annotations


def handle():
    exc = None
    try:
        raise ExceptionGroup("eg", [OSError("boom")])
    except* OSError as excs:
        exc = excs
    return exc
