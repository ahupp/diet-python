
import asyncio

async def run(arg):
    (x async for x in arg)

def main():
    try:
        asyncio.run(run(None))
    except TypeError as exc:
        return str(exc)
    return "no error"


# diet-python: validate

def validate_module(module):
    message = module.main()

    assert "__aiter__" in message
