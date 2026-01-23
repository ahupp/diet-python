def format(value):
    return None


def trigger(name):
    raise AttributeError(f"module '{__name__}' has no attribute '{name}'")

# diet-python: validate

from __future__ import annotations

import pytest
import re

def validate(module):
    expected = f"module '{module.__name__}' has no attribute 'missing'"
    with pytest.raises(AttributeError, match=re.escape(expected)):
        module.trigger("missing")
