from __future__ import annotations

import dataclasses

try:
    dataclasses.make_dataclass("C", [("for", int)])
except TypeError as exc:
    ERROR = str(exc)
else:
    ERROR = None

# diet-python: validate

from __future__ import annotations

import pytest

module = __import__("sys").modules[__name__]
assert module.ERROR == "Field names must not be keywords: 'for'"
