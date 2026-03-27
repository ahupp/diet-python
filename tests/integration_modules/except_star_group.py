from __future__ import annotations


def handle():
    exc = None
    try:
        raise ExceptionGroup("eg", [OSError("boom")])
    except* OSError as excs:
        exc = excs
    return exc

# diet-python: validate

def validate_module(module):

    exc = module.handle()
    assert exc is not None
    assert isinstance(exc, ExceptionGroup)
    assert exc.exceptions
    assert isinstance(exc.exceptions[0], OSError)
