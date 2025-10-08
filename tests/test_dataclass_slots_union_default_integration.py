from __future__ import annotations

import pytest


@pytest.mark.integration
def test_dataclass_slots_union_default_preserves_field_annotations(run_integration_module):
    with run_integration_module("dataclass_slots_union_default") as module:
        instance = module.build_example(state="ready", count=3)
    assert instance.state == "ready"
    assert instance.count == 3
