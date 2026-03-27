
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


# diet-python: validate

def validate_module(module):
    assert module.main() == [0, 2]
