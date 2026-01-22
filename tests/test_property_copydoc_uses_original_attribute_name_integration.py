from __future__ import annotations


def test_property_copydoc_uses_original_attribute_name(run_integration_module) -> None:
    with run_integration_module("property_copydoc_uses_original_attribute_name") as module:
        assert module.Derived.value.__doc__ == "base doc"
