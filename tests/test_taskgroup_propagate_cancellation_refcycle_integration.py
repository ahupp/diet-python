from __future__ import annotations

import pytest


@pytest.mark.integration
def test_taskgroup_propagate_cancellation_refcycle(run_integration_module):
    with run_integration_module("taskgroup_propagate_cancellation_refcycle") as module:
        referrers = module.run()

    assert referrers == []
