class CaptureException:
    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        self.exc = exc
        return True


def exception_context():
    def f():
        try:
            raise KeyError("a")
        except Exception:
            yield

    gen = f()
    gen.send(None)
    capture = CaptureException()
    with capture:
        gen.throw(ValueError)
    context = capture.exc.__context__
    return type(context), context.args
