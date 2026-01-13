from __future__ import annotations


def test_cleanup_dp_globals(run_integration_module):
    with run_integration_module("cleanup_dp_globals") as module:
        assert module.has_dp_name() is False
