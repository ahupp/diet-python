
def run():
    global caught
    ok = False
    try:
        raise ExceptionGroup("eg", [ValueError("boom")])
    except* ValueError as caught:
        value = caught
        ok = isinstance(value, ExceptionGroup)
    return ok

RESULT = run()
CLEARED = "caught" not in globals()


# diet-python: validate

def validate_module(module):
    assert module.RESULT is True

    assert module.CLEARED is True
