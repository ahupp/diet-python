def test_genexpr_name(run_integration_module):
    with run_integration_module("genexpr_name") as module:
        assert module.get_genexpr_name() == "<genexpr>"
