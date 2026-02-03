from tests._integration import transformed_module


def test_async_genexpr_with_async_listcomp(tmp_path):
    source = """
import asyncio

async def asynciter(seq):
    for item in seq:
        yield item

async def run():
    gen = ([i + j async for i in asynciter([1, 2])] for j in [10, 20])
    return [x async for x in gen]

def main():
    return asyncio.run(run())
"""
    with transformed_module(tmp_path, "async_genexpr_async_comp", source) as module:
        assert module.main() == [[11, 12], [21, 22]]
