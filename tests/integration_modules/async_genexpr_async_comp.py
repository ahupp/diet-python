
import asyncio

async def asynciter(seq):
    for item in seq:
        yield item

async def run():
    gen = ([i + j async for i in asynciter([1, 2])] for j in [10, 20])
    return [x async for x in gen]

def main():
    return asyncio.run(run())


# diet-python: validate

def validate_module(module):
    assert module.main() == [[11, 12], [21, 22]]
