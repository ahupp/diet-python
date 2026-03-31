import pytest

def test_eval_sees_closure_cells(run_integration_module):
    pytest.xfail("scope-aware builtin rewriting has been removed")
    with run_integration_module("eval_closure") as module:
        with pytest.raises(NotImplementedError):
            module.run()
