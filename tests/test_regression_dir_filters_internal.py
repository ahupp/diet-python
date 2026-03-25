import pytest

from tests._integration import transformed_module


def test_dir_filters_dp_internal_names(tmp_path):
    pytest.xfail("scope-aware builtin rewriting has been removed")
    source = """

def run():
    junk = 1
    return dir()
"""
    with transformed_module(tmp_path, "dir_filters", source) as module:
        with pytest.raises(NotImplementedError):
            module.run()
