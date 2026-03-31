import pytest

def test_exec_sees_locals(run_integration_module):
    pytest.xfail("scope-aware builtin rewriting has been removed")
    with run_integration_module("exec_locals") as module:
        with pytest.raises(NotImplementedError):
            module.run()
