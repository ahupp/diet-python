from contextlib import nullcontext


def unpack_starred_list():
    with nullcontext(range(1, 5)) as (a, *b, c):
        return a, b, c

# diet-python: validate

from __future__ import annotations

def validate(module):
    a, b, c = module.unpack_starred_list()
    assert a == 1
    assert b == [2, 3]
    assert c == 4
