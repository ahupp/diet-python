from tests._integration import transformed_module


def test_coroutine_return_value_preserved(tmp_path):
    source = """
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
"""

    with transformed_module(tmp_path, "coroutine_return_value", source) as module:
        assert module.main() == 1
        assert module.manual() == 1
