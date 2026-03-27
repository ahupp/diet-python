from __future__ import annotations

import functools


def build_message():
    @functools.singledispatch
    def base(arg):
        return arg

    try:
        @base.register
        def _(arg):
            return arg
    except TypeError as exc:
        return str(exc)
    raise AssertionError("expected TypeError")

# diet-python: validate

def validate_module(module):

    msg = module.build_message()
    assert "Invalid first argument to `register()`:" in msg
    assert "build_message.<locals>._" in msg
