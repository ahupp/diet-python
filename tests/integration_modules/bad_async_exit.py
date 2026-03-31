import asyncio


class BadExit:
    async def __aenter__(self):
        return "ok"

    def __aexit__(self, exc_type, exc, tb):
        return 1


async def run():
    async with BadExit():
        return "ok"


def main():
    return asyncio.run(run())


# diet-python: validate

def validate_module(module):
    import pytest

    with pytest.raises(
        TypeError,
        match=r"'async with' received an object from __aexit__ that does not implement __await__: int",
    ):
        module.main()
