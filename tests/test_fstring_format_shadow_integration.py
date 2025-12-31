from __future__ import annotations

import pytest


def test_fstring_uses_builtin_format(run_integration_module):
    with run_integration_module("fstring_format_shadow") as module:
        with pytest.raises(AttributeError, match="module 'fstring_format_shadow' has no attribute 'missing'"):
            module.trigger("missing")
