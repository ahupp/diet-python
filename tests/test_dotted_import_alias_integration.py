from __future__ import annotations

import pytest


def test_import_dotted_module_alias(run_integration_module):
    with run_integration_module("dotted_import_alias") as module:
        assert module.VALUE == "submodule"
        assert module.MODULE_NAME == "dotted_import_alias_pkg.submodule"


def test_import_stdlib_module_alias(run_integration_module):
    with run_integration_module("stdlib_import_alias") as module:
        assert module.VALUE == __import__("sys").version
