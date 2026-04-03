def test_fstring_ifexpr_interpolation_imports_and_runs(run_integration_module):
    with run_integration_module("fstring_ifexpr_interpolation") as module:
        assert module.pluralize(1) == "time"
        assert module.pluralize(2) == "times"
