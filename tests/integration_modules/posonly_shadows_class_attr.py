from __future__ import annotations


def make_value():
    class Example:
        a = 40

        def compute(self):
            def f(a, b, /):
                return a + b

            return f(1, 2)

    return Example().compute()

# diet-python: validate

from __future__ import annotations

def validate(module):
    assert module.make_value() == 3
