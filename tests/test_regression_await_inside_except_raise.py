from tests._integration import transformed_module
import asyncio


def test_await_inside_except_preserves_bare_raise(tmp_path):
    source = """
import asyncio

async def inner():
    return False

async def check():
    try:
        raise ValueError("boom")
    except Exception:
        await inner()
        raise

def run():
    return asyncio.run(check())
"""

    with transformed_module(tmp_path, "await_inside_except_raise", source) as module:
        try:
            module.run()
        except ValueError as exc:
            assert exc.args == ("boom",)
        else:  # pragma: no cover
            raise AssertionError("expected ValueError")
