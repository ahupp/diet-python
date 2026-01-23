def make_values():
    for value in (1, 2, 3):
        yield value


def forward(gen):
    yield from gen

# diet-python: validate

from __future__ import annotations

def validate(module):
    gen = module.make_values()
    forwarded = module.forward(gen)
    assert list(forwarded) == [1, 2, 3]
