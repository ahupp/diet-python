def test_while_condition_recomputed_each_iteration(run_integration_module):
    with run_integration_module("bounded_loop") as module:
        assert module.bounded_loop() == 2
