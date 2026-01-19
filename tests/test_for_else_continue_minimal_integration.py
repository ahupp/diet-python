def test_for_else_continue_minimal(run_integration_module):
    with run_integration_module("for_else_continue_minimal") as module:
        assert module.RESULT == [0, 1]
