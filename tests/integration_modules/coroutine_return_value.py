
import asyncio

async def run():
    return 1

def main():
    return asyncio.run(run())

def manual():
    coro = run()
    try:
        coro.send(None)
    except StopIteration as exc:
        return exc.value
    raise AssertionError("expected StopIteration")


# diet-python: validate

def validate_module(module):
    assert module.main() == 1

    assert module.manual() == 1
