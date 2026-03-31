import pytest

def test_import_syntax_error_propagates(run_integration_module):
    with pytest.raises(SyntaxError):
        with run_integration_module("bad_syntax"):
            pass
