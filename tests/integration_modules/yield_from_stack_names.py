import sys


def get_stack_names():
    def f():
        frame = sys._getframe()
        yield (frame.f_code.co_name, frame.f_back.f_code.co_name)

    def g():
        yield from f()

    gen = g()
    return gen.send(None)

# diet-python: validate

import pytest

module = __import__("sys").modules[__name__]
if __dp_integration_mode__ == "eval":
    pytest.xfail("sys._getframe unsupported in eval mode")
assert module.get_stack_names() == ("f", "g")
