from __future__ import annotations

import pytest


def test_future_import_invalid_feature_raises_syntaxerror(run_integration_module):
    with pytest.raises(SyntaxError) as excinfo:
        with run_integration_module("future_import_invalid_feature"):
            pass
    assert "not_a_feature" in str(excinfo.value)
