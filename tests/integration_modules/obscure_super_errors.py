from __future__ import annotations


def exercise():
    class X:
        def f(x):
            nonlocal __class__
            del __class__
            super()

    try:
        X().f()
    except Exception as exc:
        return type(exc), str(exc)

    return None, None

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
exc_type, message = module.exercise()
assert exc_type is RuntimeError
assert "empty __class__ cell" in message
