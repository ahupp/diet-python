from __future__ import annotations


def handle():
    captured = None
    try:
        raise ExceptionGroup("eg", [OSError("boom")])
    except* OSError as excs:
        captured = excs
    return captured

# diet-python: validate

def validate_module(module):

    exc = module.handle()
    assert exc is not None
    assert isinstance(exc, ExceptionGroup)
    assert isinstance(exc.exceptions[0], OSError)
