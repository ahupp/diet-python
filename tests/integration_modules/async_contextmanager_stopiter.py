import asyncio
from contextlib import asynccontextmanager


@asynccontextmanager
async def cm():
    yield


async def runner():
    try:
        async with cm():
            raise StopIteration("spam")
    except Exception as exc:
        return type(exc), exc


def check():
    return asyncio.run(runner())

# diet-python: validate

module = __import__("sys").modules[__name__]
exc_type, exc = module.check()
assert exc_type is StopIteration
assert exc.args == ("spam",)
