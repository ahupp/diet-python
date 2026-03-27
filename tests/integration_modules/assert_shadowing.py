from __future__ import annotations


def trigger():
    global AssertionError
    AssertionError = TypeError
    try:
        assert False, "hello"
    except BaseException as exc:
        del AssertionError
        return exc
    else:
        del AssertionError
        raise AssertionError("missing exception")

# diet-python: validate

def validate_module(module):

    exc = module.trigger()
    assert isinstance(exc, AssertionError)
    assert str(exc) == "hello"
