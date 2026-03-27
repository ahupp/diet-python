from __future__ import annotations

import dataclasses

try:
    dataclasses.make_dataclass("C", [("for", int)])
except TypeError as exc:
    ERROR = str(exc)
else:
    ERROR = None

# diet-python: validate

def validate_module(module):

    import pytest

    assert module.ERROR == "Field names must not be keywords: 'for'"
