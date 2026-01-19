def test_support_current_exception_recursion_minimal(run_integration_module):
    with run_integration_module("support_current_exception_recursion_minimal") as module:
        assert module.RESULT is True
