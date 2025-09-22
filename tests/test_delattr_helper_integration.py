from __future__ import annotations

import pytest


@pytest.mark.integration
def test_delattr_helper_supported(run_integration_module) -> None:
    with run_integration_module("delattr_missing") as module:
        assert module.ATTRIBUTE_DELETED is True
