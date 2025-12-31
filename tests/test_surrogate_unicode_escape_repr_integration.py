def test_surrogate_unicode_escape_repr(run_integration_module):
    with run_integration_module("surrogate_unicode_escape_repr") as module:
        assert module.repr_value() == "'\\udcba'"
        assert module.ascii_value() == "'\\udcba'"
