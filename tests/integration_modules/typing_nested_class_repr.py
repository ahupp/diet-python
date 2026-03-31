"""Demonstrates loss of the enclosing class name in nested class repr."""

from typing import Any

class Container:
    def make(self) -> str:
        class Sub(Any):
            pass
        return repr(Sub)

VALUE = Container().make()

# diet-python: validate

def validate_module(module):
    assert "Container.make.<locals>.Sub" in module.VALUE
