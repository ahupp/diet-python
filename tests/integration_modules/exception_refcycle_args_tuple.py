import gc


def run():
    exc = None
    try:
        raise RuntimeError("boom")
    except RuntimeError as err:
        exc = err
    return gc.get_referrers(exc)

# diet-python: validate

def validate_module(module):

    assert module.run() == []
