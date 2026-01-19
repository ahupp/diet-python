
def test_compare_in_while(run_integration_module):
    with run_integration_module("compare_in_while") as module:
        assert module.loop_compare(1, 1) is True
        assert module.loop_compare(1, 2) is False
