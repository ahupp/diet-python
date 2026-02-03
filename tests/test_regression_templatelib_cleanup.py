from tests._integration import transformed_module


def test_templatelib_import_retained(tmp_path):
    source = """
from string.templatelib import Template

def make():
    return t"{1}"
"""
    with transformed_module(tmp_path, "templatelib_cleanup", source) as module:
        result = module.make()
        assert isinstance(result, module.Template)
