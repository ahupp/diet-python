from __future__ import annotations


def test_import_dotted_module_alias(run_integration_module):
    with run_integration_module("dotted_import_alias") as module:
        assert module.VALUE == "submodule"
        assert module.MODULE_NAME == "dotted_import_alias_pkg.submodule"
