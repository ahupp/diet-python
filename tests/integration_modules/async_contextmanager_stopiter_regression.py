
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

def validate_module(module):
    exc_type, exc = module.check()

    assert exc_type is StopIteration

    assert exc.args == ("spam",)
