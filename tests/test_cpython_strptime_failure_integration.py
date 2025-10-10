from __future__ import annotations

import pytest


def test_cpython_strptime_failure_raises_type_error(run_integration_module):
    with run_integration_module("cpython_strptime_failure") as module:
        with pytest.raises(TypeError, match="callable"):
            module.parse_invalid_offset()
