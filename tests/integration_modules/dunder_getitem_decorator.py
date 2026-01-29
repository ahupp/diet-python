def call_original(func):
    def inner(*args, **kwargs):
        return func(*args, **kwargs)
    return inner


class Example:
    @call_original
    def __getitem__(self, item):
        return item

# diet-python: validate

import pytest

module = __import__("sys").modules[__name__]
for item in [1]:
    example = module.Example()
    assert example[item] == item
