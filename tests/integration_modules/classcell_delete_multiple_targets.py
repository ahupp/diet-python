from __future__ import annotations


def exercise():
    class X:
        def f(x):
            nonlocal __class__
            marker = "ok"
            del __class__, marker

            try:
                marker
            except Exception as exc:
                marker_exc = type(exc), str(exc)
            else:
                marker_exc = None, None

            try:
                super()
            except Exception as exc:
                class_exc = type(exc), str(exc)
            else:
                class_exc = None, None

            return marker_exc, class_exc

    return X().f()


# diet-python: validate

marker_exc, class_exc = exercise()
marker_type, marker_message = marker_exc
class_type, class_message = class_exc

assert marker_type is UnboundLocalError
assert "marker" in marker_message
assert class_type is RuntimeError
assert "empty __class__ cell" in class_message
