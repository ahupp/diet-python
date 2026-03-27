import asyncio


class Once:
    def __await__(self):
        yield "tick"
        return 41


async def run():
    value = await Once()
    return value + 1

# diet-python: validate

def validate_module(module):
    import asyncio

    coro = module.run()
    assert asyncio.iscoroutine(coro)
    assert coro.send(None) == "tick"
    try:
        coro.send(None)
    except StopIteration as exc:
        assert exc.value == 42
    else:
        raise AssertionError("expected StopIteration")

    assert asyncio.iscoroutinefunction(module.run)
