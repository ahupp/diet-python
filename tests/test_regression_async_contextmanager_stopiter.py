from tests._integration import transformed_module


def test_async_contextmanager_stopiter_regression(tmp_path):
    source = """
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
"""

    with transformed_module(
        tmp_path, "async_contextmanager_stopiter_regression", source
    ) as module:
        exc_type, exc = module.check()
        assert exc_type is StopIteration
        assert exc.args == ("spam",)
