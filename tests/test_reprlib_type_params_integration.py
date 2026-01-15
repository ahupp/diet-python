def test_reprlib_type_params_integration(run_integration_module):
    with run_integration_module("reprlib_type_params") as module:
        assert module.RESULT == ""
