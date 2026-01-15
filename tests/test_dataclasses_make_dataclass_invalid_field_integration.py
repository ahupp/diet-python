from __future__ import annotations

import pytest


@pytest.mark.integration
def test_dataclasses_make_dataclass_invalid_field(run_integration_module):
    with run_integration_module("dataclasses_make_dataclass_invalid_field") as module:
        assert module.ERROR == "Field names must not be keywords: 'for'"
