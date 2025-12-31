from __future__ import annotations


def test_enum_ignore_dynamic_names(run_integration_module):
    with run_integration_module("enum_ignore_dynamic_names") as module:
        period = module.Period
        assert period.OneDay is period.day_1
        assert period.OneDay.value == 1
