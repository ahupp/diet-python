from tests._integration import transformed_module


def test_async_genexpr_await_in_filter(tmp_path):
    source = """
import asyncio

async def pred(i):
    await asyncio.sleep(0)
    return i % 2 == 0

async def run():
    gen = (i for i in range(4) if await pred(i))
    out = []
    async for item in gen:
        out.append(item)
    return out

def main():
    return asyncio.run(run())
"""
    with transformed_module(tmp_path, "genexpr_async_await", source) as module:
        assert module.main() == [0, 2]


def test_async_genexpr_checks_aiter_eagerly(tmp_path):
    source = """
import asyncio

async def run(arg):
    (x async for x in arg)

def main():
    try:
        asyncio.run(run(None))
    except TypeError as exc:
        return str(exc)
    return "no error"
"""
    with transformed_module(tmp_path, "genexpr_async_aiter", source) as module:
        message = module.main()
        assert "__aiter__" in message
