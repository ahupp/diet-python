import dataclasses


@dataclasses.dataclass(slots=True)
class Example:
    label: str
    state: str | None = None
    count: int = 0


def build_example(**kwargs):
    return Example("label", **kwargs)

# diet-python: validate

from __future__ import annotations

import pytest

module = __import__("sys").modules[__name__]
instance = module.build_example(state="ready", count=3)
assert instance.state == "ready"
assert instance.count == 3
