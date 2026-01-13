def test_nested_async_comprehension_integration(run_integration_module):
    with run_integration_module("nested_async_comprehension") as module:
        assert module.get_values() == [[11, 12], [21, 22]]
        assert module.get_gen_values() == [0, 1, 2, 0, 1, 2, 3, 4]
