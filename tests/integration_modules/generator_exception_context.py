from __future__ import annotations


def exercise():
    def f():
        try:
            raise KeyError("a")
        except Exception:
            yield

    gen = f()
    gen.send(None)
    try:
        gen.throw(ValueError)
    except Exception as exc:
        context = exc.__context__
        return type(context), getattr(context, "args", None)
    return None, None
