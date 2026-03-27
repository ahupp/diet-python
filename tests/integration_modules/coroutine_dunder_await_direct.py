
class Once:
    def __await__(self):
        if False:
            yield None
        return 42

def direct():
    it = Once().__await__()
    try:
        it.send(None)
    except StopIteration as exc:
        return exc.value
    raise AssertionError("expected StopIteration")


# diet-python: validate

def validate_module(module):
    assert module.direct() == 42
