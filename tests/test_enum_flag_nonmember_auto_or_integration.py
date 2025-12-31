from __future__ import annotations


def test_enum_flag_nonmember_auto_or(run_integration_module):
    with run_integration_module("enum_flag_nonmember_auto_or") as module:
        a, b, all_value = module.build_values()
        assert a.value == 1
        assert b.value == 2
        assert all_value == 3
