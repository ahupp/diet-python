import pytest

from tests._integration import transformed_module


def test_if_empty_body_preserves_truthiness(tmp_path):
    source = """

def run():
    if NotImplemented:
        pass
"""
    with transformed_module(tmp_path, "truthiness_notimplemented", source) as module:
        with pytest.raises(TypeError, match="NotImplemented should not be used in a boolean context"):
            module.run()
