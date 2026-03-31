import pytest

def test_dir_filters_dp_internal_names(run_integration_module):
    pytest.xfail("scope-aware builtin rewriting has been removed")
    with run_integration_module("dir_filters") as module:
        with pytest.raises(NotImplementedError):
            module.run()
