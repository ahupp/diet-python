import gc


def run():
    exc = None
    try:
        raise RuntimeError("boom")
    except RuntimeError as e:
        exc = e
    return gc.get_referrers(exc)

# diet-python: validate

def validate_module(module):

    import pytest

    referrers = module.run()

    assert referrers == []
