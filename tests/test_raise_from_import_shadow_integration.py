from __future__ import annotations


def test_raise_from_import_shadow(run_integration_module):
    with run_integration_module("raise_from_import_shadow") as module:
        assert module.ASYNCIO_SHADOWED is False
