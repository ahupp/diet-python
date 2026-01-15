from __future__ import annotations


def test_with_extended_targets(run_integration_module):
    with run_integration_module("with_extended_targets") as module:
        a, b, c = module.unpack_starred_list()
        assert a == 1
        assert b == [2, 3]
        assert c == 4
