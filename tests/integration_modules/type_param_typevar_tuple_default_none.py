class A[*Ts]:
    pass

# diet-python: disabled (segfaults; re-enable later)

from __future__ import annotations

def validate(module):
    assert module.A.__name__ == "A"
