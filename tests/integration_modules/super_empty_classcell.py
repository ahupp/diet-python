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
