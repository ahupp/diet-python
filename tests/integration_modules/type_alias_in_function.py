from typing import Callable


def make_alias():
    type X[**P] = Callable[P, int]
    return X

# diet-python: validate

module = __import__("sys").modules[__name__]
alias = module.make_alias()
assert alias.__name__ == "X"
