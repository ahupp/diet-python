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


# diet-python: validate

def validate_module(module):
    try:
        module.run()
    except ValueError as exc:
        assert exc.args == ("boom",)
    else:  # pragma: no cover
        raise AssertionError("expected ValueError")
