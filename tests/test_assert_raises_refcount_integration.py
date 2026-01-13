def test_assert_raises_refcount_integration(run_integration_module):
    with run_integration_module("assert_raises_refcount") as module:
        before, after = module.run()
        assert before == after
