class Example:
    def __secret(self):
        return "payload"

    def reveal(self):
        return self.__secret()

RESULT = Example().reveal()

# diet-python: validate

def validate_module(module):
    import pytest

    assert module.RESULT == "payload"
