def format(value):
    return None


def trigger(name):
    raise AttributeError(f"module '{__name__}' has no attribute '{name}'")

# diet-python: validate

from __future__ import annotations

import pytest

def validate(module):
    with pytest.raises(AttributeError, match="module 'fstring_format_shadow' has no attribute 'missing'"):
        module.trigger("missing")
