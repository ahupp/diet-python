from __future__ import annotations

import pytest


def test_method_local_shadowing_raises_name_error(run_integration_module):
    with run_integration_module("method_local_shadowing") as module:
        instance = module.Example()
        with pytest.raises(NameError, match="run"):
            instance.run()
