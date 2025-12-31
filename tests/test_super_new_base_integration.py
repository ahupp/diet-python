from __future__ import annotations


def test_super_in_base_new_uses_defining_class(run_integration_module):
    with run_integration_module("super_new_base") as module:
        instance = module.build_child()
        assert instance.a == 1
        assert instance.b == 1
