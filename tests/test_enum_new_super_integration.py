from __future__ import annotations

import pytest


def test_enum_new_super_raises_type_error(run_integration_module):
    with run_integration_module("enum_new_super") as module:
        with pytest.raises(TypeError, match="do not use `super\\(\\).__new__"):
            module.build_enum()
