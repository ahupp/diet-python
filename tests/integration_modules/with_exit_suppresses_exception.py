class Suppress:
    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        return True


def run():
    with Suppress():
        raise RuntimeError("boom")
    return "ok"


RESULT = run()

# diet-python: validate

def validate(module):
    assert module.RESULT == "ok"
