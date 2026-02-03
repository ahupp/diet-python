from tests._integration import transformed_module


def test_property_subclass_docstring_preserved(tmp_path):
    source = """
class PropertySub(property):
    '''This is a subclass of property'''

def get_doc():
    return PropertySub.__doc__
"""
    with transformed_module(tmp_path, "property_sub_doc", source) as module:
        assert module.get_doc() == "This is a subclass of property"
