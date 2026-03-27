import asyncio
import gc


def run():
    exc = None
    try:
        try:
            raise asyncio.CancelledError()
        except asyncio.CancelledError as err:
            raise TimeoutError from err
    except TimeoutError as err:
        exc = err.__cause__
    return gc.get_referrers(exc)

# diet-python: validate

def validate_module(module):

    import pytest

    referrers = module.run()

    assert referrers == []
