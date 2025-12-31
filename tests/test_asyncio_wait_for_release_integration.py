from __future__ import annotations


def test_asyncio_wait_for_releases_coroutine_locals(run_integration_module):
    with run_integration_module("asyncio_wait_for_release") as module:
        assert module.leak_check() is None
