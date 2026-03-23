from tests._integration import transformed_module


def test_direct_module_init_loader_preserves_module_docstring(tmp_path):
    source = '''"""module docs"""

VALUE = 1
'''

    with transformed_module(tmp_path, "module_docstring_direct_loader", source) as module:
        assert module.__doc__ == "module docs"
        assert module.VALUE == 1
