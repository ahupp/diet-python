from __future__ import annotations


def test_asyncio_taskgroup_base_error_refcycle(run_integration_module):
    with run_integration_module("asyncio_taskgroup_base_error_refcycle") as module:
        assert module.referrer_frames() == []
