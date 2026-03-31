import pytest

def test_dictcomp_helper_preserves_result_container(run_integration_module):
    with run_integration_module("dictcomp_temp_collision") as module:
        assert module.dict_comp_fib() == {
            1: 2,
            2: 3,
            3: 5,
            5: 8,
            8: 13,
            13: 21,
        }


def test_dictcomp_helper_works_in_class_namespace(run_integration_module):
    pytest.xfail("scope-aware builtin rewriting has been removed")
    with run_integration_module("dictcomp_temp_collision_class") as module:
        assert [member.name for member in module.Foo] == ["FOO_CAT", "FOO_HORSE"]
