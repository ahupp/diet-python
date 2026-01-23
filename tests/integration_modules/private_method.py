class Example:
    def __secret(self):
        return "payload"

    def reveal(self):
        return self.__secret()

RESULT = Example().reveal()

# diet-python: validate

import pytest

def validate(module):
    assert module.RESULT == "payload"
