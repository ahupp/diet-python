def test_async_with_missing_aexit_integration(run_integration_module):
    with run_integration_module("async_with_missing_aexit") as module:
        message = module.get_result()
        assert "asynchronous context manager" in message
        assert "__aexit__" in message
