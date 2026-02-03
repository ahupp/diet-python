import pytest

from tests._integration import transformed_module


def test_import_syntax_error_propagates(tmp_path):
    source = "a = 1 b = 2"
    with pytest.raises(SyntaxError):
        with transformed_module(tmp_path, "bad_syntax", source):
            pass
