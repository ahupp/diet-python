def test_with_exit_suppresses_exception_integration(run_integration_module):
    with run_integration_module("with_exit_suppresses_exception") as module:
        assert module.RESULT == "ok"
