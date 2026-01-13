def test_async_with_nonawaitable_aenter_integration(run_integration_module):
    with run_integration_module("async_with_nonawaitable_aenter") as module:
        message = module.get_error()
        assert "__aenter__" in message
        assert "__await__" in message
