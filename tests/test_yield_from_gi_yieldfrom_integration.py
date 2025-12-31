def test_yield_from_gi_yieldfrom(run_integration_module):
    with run_integration_module("yield_from_gi_yieldfrom") as module:
        assert module.get_yieldfrom_name() == "a"
