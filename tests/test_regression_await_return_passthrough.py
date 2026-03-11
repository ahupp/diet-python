from tests._integration import transformed_module


def test_await_uses_coroutine_result_not_stopiteration(tmp_path):
    source = """
import asyncio

async def inner():
    return False

async def outer():
    try:
        value = await inner()
        return ("ok", value)
    except Exception as exc:
        return (type(exc).__name__, exc.args)

def check():
    return asyncio.run(outer())
"""

    with transformed_module(tmp_path, "await_return_passthrough", source) as module:
        assert module.check() == ("ok", False)
