import asyncio


class BadEnter:
    def __aenter__(self):
        return 1

    async def __aexit__(self, exc_type, exc, tb):
        return False


async def run():
    async with BadEnter():
        return "ok"


def main():
    return asyncio.run(run())


# diet-python: validate

def validate_module(module):
    import pytest

    with pytest.raises(
        TypeError,
        match=r"'async with' received an object from __aenter__ that does not implement __await__: int",
    ):
        module.main()
