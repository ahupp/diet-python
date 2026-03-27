def call_original(func):
    def inner(*args, **kwargs):
        return func(*args, **kwargs)
    return inner


class Example:
    @call_original
    def __getitem__(self, item):
        return item

# diet-python: validate

def validate_module(module):
    import pytest

    for item in [1]:
        example = module.Example()
        assert example[item] == item
