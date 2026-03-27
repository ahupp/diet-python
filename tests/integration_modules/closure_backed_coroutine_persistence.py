
class Once:
    def __await__(self):
        yield "tick"
        return 4

def make_runner(base):
    async def run():
        total = base
        total += await Once()
        return total
    return run

def manual():
    coro = make_runner(3)()
    assert coro.send(None) == "tick"
    try:
        coro.send(None)
    except StopIteration as exc:
        return exc.value
    raise AssertionError("expected StopIteration")


# diet-python: validate

def validate_module(module):
    assert module.manual() == 7
