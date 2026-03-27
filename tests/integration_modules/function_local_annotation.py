from __future__ import annotations


def exercise():
    def inner():
        value: int = 1
        return value

    return inner()

# diet-python: validate

def validate_module(module):

    assert module.exercise() == 1
