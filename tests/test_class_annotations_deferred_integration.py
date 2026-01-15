from __future__ import annotations

import pytest


def test_class_annotations_deferred(run_integration_module):
    with run_integration_module("class_annotations_deferred") as module:
        with pytest.raises(NameError):
            _ = module.ThemeSection.__annotations__
