from __future__ import annotations


def test_enum_dynamic_members_vars_update(run_integration_module):
    with run_integration_module("enum_dynamic_members_vars_update") as module:
        foo = module.Foo
        assert list(foo) == [foo.FOO_CAT, foo.FOO_HORSE]
        assert foo.FOO_CAT.value == "aloof"
        assert foo.FOO_HORSE.upper() == "BIG"
