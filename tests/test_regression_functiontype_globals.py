import pytest

def test_functiontype_injects_dp_globals(run_integration_module):
    pytest.xfail(
        "FunctionType(code, globals) drops hidden SOAC kwdefaults used by def_fn wrappers"
    )
    with run_integration_module("functiontype_globals") as module:
        assert module.run() == 2
