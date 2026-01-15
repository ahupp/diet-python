def test_iter_refcount_behavior_integration(run_integration_module):
    with run_integration_module("iter_refcount_behavior") as module:
        assert module.RESULT == 0
