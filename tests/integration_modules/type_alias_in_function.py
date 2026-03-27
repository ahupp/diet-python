from typing import Callable


def make_alias():
    type X[**P] = Callable[P, int]
    return X

# diet-python: validate

def validate_module(module):
    alias = module.make_alias()
    assert alias.__name__ == "X"
