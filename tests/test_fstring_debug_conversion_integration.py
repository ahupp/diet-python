def test_fstring_debug_conversion(run_integration_module):
    with run_integration_module("fstring_debug_conversion") as module:
        assert module.format_debug() == "value='A string'"
