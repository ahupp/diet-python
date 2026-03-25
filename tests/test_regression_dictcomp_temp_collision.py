import pytest

from tests._integration import transformed_module


def test_dictcomp_helper_preserves_result_container(tmp_path):
    source = """
def dict_comp_fib():
    a, b = 1, 2
    fib = {(c := a): (a := b) + (b := a + c) - b for __ in range(6)}
    return fib
"""

    with transformed_module(tmp_path, "dictcomp_temp_collision", source) as module:
        assert module.dict_comp_fib() == {
            1: 2,
            2: 3,
            3: 5,
            5: 8,
            8: 13,
            13: 21,
        }


def test_dictcomp_helper_works_in_class_namespace(tmp_path):
    pytest.xfail("scope-aware builtin rewriting has been removed")
    source = """
from enum import Enum

FOO_DEFINES = {
    "FOO_CAT": "aloof",
    "BAR_DOG": "friendly",
    "FOO_HORSE": "big",
}

class Foo(Enum):
    vars().update({
        k: v
        for k, v in FOO_DEFINES.items()
        if k.startswith("FOO_")
    })
"""

    with transformed_module(tmp_path, "dictcomp_temp_collision_class", source) as module:
        assert [member.name for member in module.Foo] == ["FOO_CAT", "FOO_HORSE"]
