from __future__ import annotations

import pytest


@pytest.mark.integration
def test_exception_refcycle_after_except(run_integration_module):
    with run_integration_module("exception_refcycle_after_except") as module:
        referrers = module.run()

    assert referrers == []
