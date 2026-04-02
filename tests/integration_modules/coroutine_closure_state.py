import asyncio


class Once:
    def __await__(self):
        yielded = yield "tick"
        return yielded if yielded is not None else 5


def make_runner(delta):
    outer = delta

    async def run():
        total = 1
        total += outer
        total += await Once()
        return total

    return run()

# diet-python: validate

def validate_module(module):
    coro = module.make_runner(3)
    if type(coro).__name__ == "Coroutine":
        assert coro.cr_frame is None
    else:
        assert coro.cr_frame is not None
    assert coro.send(None) == "tick"
    try:
        coro.send(7)
    except StopIteration as exc:
        assert exc.value == 11
    else:
        raise AssertionError("expected StopIteration")
