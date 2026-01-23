def comp_scope():
    a = 1
    values = [(a := a + 1) for _ in range(2)]
    return a, values


def genexpr_scope():
    a = 1
    gen = (b := a + i for i in range(2))
    return a, list(gen), b

# diet-python: validate

from __future__ import annotations

def validate(module):
    a, values = module.comp_scope()
    assert a == 3
    assert values == [2, 3]

    a2, values2, b2 = module.genexpr_scope()
    assert a2 == 1
    assert values2 == [1, 2]
    assert b2 == 2
