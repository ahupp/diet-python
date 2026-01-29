class Example:
    def __secret(self):
        return "payload"

    def reveal(self):
        return self.__secret()

RESULT = Example().reveal()

# diet-python: validate

import pytest

module = __import__("sys").modules[__name__]
assert module.RESULT == "payload"
