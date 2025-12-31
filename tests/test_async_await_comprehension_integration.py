import asyncio


def test_async_await_comprehension_executes(run_integration_module):
    with run_integration_module("async_await_comprehension") as module:
        assert asyncio.run(module.gather_once()) == [1]
