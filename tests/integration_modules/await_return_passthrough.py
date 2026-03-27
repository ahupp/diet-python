
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


# diet-python: validate

def validate_module(module):
    assert module.check() == ("ok", False)
