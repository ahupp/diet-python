import asyncio

from tests._integration import transformed_module


def test_asyncio_call_graph_handles_async_gen(tmp_path):
    source = """
import asyncio

def run():
    async def gen_nested_call():
        task = asyncio.current_task()
        stack = asyncio.capture_call_graph(task, depth=1)
        for entry in stack.call_stack:
            gen = entry.frame.f_generator
            if gen is None:
                continue
            if hasattr(gen, "cr_code"):
                _ = gen.cr_code
            else:
                _ = gen.ag_code
        return True

    async def gen():
        for num in range(2):
            yield num
            if num == 1:
                await gen_nested_call()

    async def main():
        async for _ in gen():
            pass
        return True

    return asyncio.run(main())
"""
    with transformed_module(tmp_path, "asyncio_call_graph_async_gen", source) as module:
        assert module.run() is True
