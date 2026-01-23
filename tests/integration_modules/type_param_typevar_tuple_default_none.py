class A[*Ts]:
    pass

# diet-python: validate

from __future__ import annotations

def validate(module):
    assert module.A.__name__ == "A"
