from __future__ import annotations

import pytest


def test_missing_from_import_raises_importerror(run_integration_module):
    with run_integration_module("missing_from_import") as module:
        assert module.RESULT == "fallback"
