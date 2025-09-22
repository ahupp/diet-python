from __future__ import annotations

import pytest


def test_class_body_uses_globals_when_method_shares_name(run_integration_module):
    with pytest.raises(UnboundLocalError):
        with run_integration_module("method_name_clash"):
            pass

