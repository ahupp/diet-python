from __future__ import annotations


def test_support_import_internalcapi_integration(run_integration_module):
    with run_integration_module("support_import_internalcapi") as module:
        assert module.exercise() == "ok"
