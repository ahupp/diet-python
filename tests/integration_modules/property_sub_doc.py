
class PropertySub(property):
    '''This is a subclass of property'''

def get_doc():
    return PropertySub.__doc__


# diet-python: validate

def validate_module(module):
    assert module.get_doc() == "This is a subclass of property"
