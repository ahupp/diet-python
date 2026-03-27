from typing import TypeVar


RESULT = TypeVar("Result")

# diet-python: validate

def validate_module(module):

    assert module.RESULT.__name__ == "Result"
