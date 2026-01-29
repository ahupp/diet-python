def make_values():
    for value in (1, 2, 3):
        yield value


def forward(gen):
    yield from gen

# diet-python: validate

from __future__ import annotations

module = __import__("sys").modules[__name__]
gen = module.make_values()
forwarded = module.forward(gen)
assert list(forwarded) == [1, 2, 3]
