from __future__ import annotations

def test_property_setter_roundtrip(run_integration_module) -> None:
    """Property setters should round-trip values under the transform."""
    with run_integration_module("property_setter") as module:
        instance = module.Example()
        instance.value = 5
        assert instance.value == 5
