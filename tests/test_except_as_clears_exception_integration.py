from __future__ import annotations


def test_except_as_clears_exception_reference(run_integration_module):
    with run_integration_module("except_as_clears_exception") as module:
        assert module.count_exception_referrer_frames() == 0
