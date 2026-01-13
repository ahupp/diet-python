def test_async_for_missing_aiter_integration(run_integration_module):
    with run_integration_module("async_for_missing_aiter") as module:
        message = module.get_error()
        assert "async for" in message
        assert "__aiter__" in message
