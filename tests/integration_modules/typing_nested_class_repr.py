"""Demonstrates loss of the enclosing class name in nested class repr."""

from typing import Any

class Container:
    def make(self) -> str:
        class Sub(Any):
            pass
        return repr(Sub)

VALUE = Container().make()
