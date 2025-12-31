def test_yield_from_stack_names(run_integration_module):
    with run_integration_module("yield_from_stack_names") as module:
        assert module.get_stack_names() == ("f", "g")
